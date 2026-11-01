//! Integration tests for Distributed Task Mesh.
//!
//! Validates cluster joining, task scheduling, WAL persistence, and TCP client/server interactions.

use distributed_task_mesh::*;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

#[tokio::test]
async fn test_work_stealing_scheduler_under_concurrent_load() {
    let scheduler = WorkStealingScheduler::new();
    let worker1 = scheduler.register_worker(NodeId::new("worker-1"));
    let worker2 = scheduler.register_worker(NodeId::new("worker-2"));
    let worker3 = scheduler.register_worker(NodeId::new("worker-3"));

    // Overload worker 1 with 100 tasks directly
    for i in 0..100 {
        worker1.push(TaskDefinition {
            id: TaskId::new(),
            name: format!("heavy_task_{i}"),
            payload: serde_json::json!({ "index": i }),
            priority: if i % 10 == 0 {
                Priority::High
            } else {
                Priority::Normal
            },
            max_retries: 3,
            timeout_ms: 5000,
            created_at: 0,
        });
    }

    assert_eq!(worker1.len(), 100);
    assert_eq!(worker2.len(), 0);
    assert_eq!(worker3.len(), 0);

    // Worker 2 and 3 should steal from worker 1
    let mut stolen_by_w2 = 0;
    while let Some((_, stolen)) = scheduler.next_task_or_steal(&NodeId::new("worker-2")) {
        if stolen {
            stolen_by_w2 += 1;
        }
        if worker1.len() <= 50 {
            break;
        }
    }

    assert!(
        stolen_by_w2 > 0,
        "Worker 2 must successfully steal tasks from overloaded Worker 1"
    );
    assert!(
        scheduler.stolen_count() > 0,
        "Scheduler stolen_count metric must increment"
    );
}

#[tokio::test]
async fn test_wal_durability_and_crash_recovery() {
    let tmp = tempdir().unwrap();
    let wal_path = tmp.path().join("mesh.wal");

    let task_id1 = TaskId::new();
    let task_id2 = TaskId::new();

    // 1. First node session: writes to WAL and completes task 1
    {
        let node = TaskMeshNode::new(NodeId::new("primary"), &wal_path).unwrap();
        node.scheduler.register_worker(NodeId::new("w1"));

        node.submit(TaskDefinition {
            id: task_id1,
            name: "task_1".to_string(),
            payload: serde_json::json!({ "action": "encode" }),
            priority: Priority::Normal,
            max_retries: 3,
            timeout_ms: 1000,
            created_at: 100,
        })
        .unwrap();

        node.submit(TaskDefinition {
            id: task_id2,
            name: "task_2".to_string(),
            payload: serde_json::json!({ "action": "render" }),
            priority: Priority::High,
            max_retries: 3,
            timeout_ms: 1000,
            created_at: 101,
        })
        .unwrap();

        node.complete_task(task_id1, serde_json::json!({ "status": "ok" }), 42)
            .unwrap();
    }

    // 2. Simulate crash and restart: create a new node pointing to the existing WAL
    {
        let node_recovered =
            TaskMeshNode::new(NodeId::new("primary_recovered"), &wal_path).unwrap();

        let s1 = node_recovered
            .get_status(&task_id1)
            .expect("Task 1 must exist in recovered state");
        let s2 = node_recovered
            .get_status(&task_id2)
            .expect("Task 2 must exist in recovered state");

        assert_eq!(
            s1,
            TaskStatus::Completed {
                result: serde_json::json!({ "status": "ok" }),
                duration_ms: 42,
            }
        );
        assert_eq!(s2, TaskStatus::Pending);
    }
}

#[tokio::test]
async fn test_cluster_membership_and_failure_detection() {
    let membership = ClusterMembership::new(NodeId::new("node-alpha"), Duration::from_millis(50));
    let peer = NodeId::new("node-beta");

    // Heartbeat from peer
    membership.record_heartbeat(peer.clone(), 0.15);
    assert_eq!(membership.active_nodes_count(), 2);

    // Sleep past dead timeout
    tokio::time::sleep(Duration::from_millis(120)).await;

    let (_suspect, dead) = membership.audit_membership();
    assert!(
        dead.contains(&peer),
        "Node Beta must be classified as Dead after heartbeat timeout"
    );

    // Leader election
    let (term, leader) = membership.elect_leader(NodeId::new("node-alpha"));
    assert_eq!(term, 1);
    assert_eq!(leader, NodeId::new("node-alpha"));
    assert_eq!(membership.leader(), Some(NodeId::new("node-alpha")));
}

#[tokio::test]
async fn test_tcp_server_protocol_roundtrip() {
    let tmp = tempdir().unwrap();
    let wal_path = tmp.path().join("server.wal");

    let node = Arc::new(TaskMeshNode::new(NodeId::new("server-node"), &wal_path).unwrap());
    node.scheduler.register_worker(NodeId::new("srv-w1"));

    let server = TaskMeshServer::new(Arc::clone(&node), "127.0.0.1:28472");
    tokio::spawn(async move {
        server.run().await.unwrap();
    });

    // Wait for server to bind
    tokio::time::sleep(Duration::from_millis(100)).await;

    let stream = TcpStream::connect("127.0.0.1:28472").await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    // 1. Test PING
    writer.write_all(b"PING\n").await.unwrap();
    let resp = lines.next_line().await.unwrap().unwrap();
    assert_eq!(resp, "PONG");

    // 2. Test SUBMIT
    writer
        .write_all(b"SUBMIT {\"image\": \"alpine\", \"tag\": \"latest\"}\n")
        .await
        .unwrap();
    let submit_resp_str = lines.next_line().await.unwrap().unwrap();
    let submit_resp: serde_json::Value = serde_json::from_str(&submit_resp_str).unwrap();
    assert_eq!(submit_resp["status"], "submitted");
    let task_id = submit_resp["task_id"].as_str().unwrap();

    // 3. Test POLL
    writer
        .write_all(format!("POLL {}\n", task_id).as_bytes())
        .await
        .unwrap();
    let poll_resp_str = lines.next_line().await.unwrap().unwrap();
    let poll_resp: serde_json::Value = serde_json::from_str(&poll_resp_str).unwrap();
    assert_eq!(poll_resp["task_id"], task_id);
    assert_eq!(poll_resp["status"], "Pending");

    // 4. Test METRICS
    writer.write_all(b"METRICS\n").await.unwrap();
    let metrics_resp_str = lines.next_line().await.unwrap().unwrap();
    let metrics_resp: serde_json::Value = serde_json::from_str(&metrics_resp_str).unwrap();
    assert_eq!(metrics_resp["tasks_submitted"], 1);

    // 5. Test QUIT
    writer.write_all(b"QUIT\n").await.unwrap();
}
