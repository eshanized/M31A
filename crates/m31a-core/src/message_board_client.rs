//! Typed client for inter-agent message boards and live notifications.
//!
//! Supports topic channels, inter-agent broadcasting, and asynchronous notification delivery.

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use uuid::Uuid;

/// Channel descriptor in the agent message board.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChannelInfo {
    pub name: String,
    pub topic: Option<String>,
    pub created_at: String,
}

/// Thread summary in a message board channel.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BoardThreadSummary {
    pub id: String,
    pub channel: String,
    pub title: String,
    pub author: String,
    pub created_at: String,
    pub post_count: usize,
}

/// A single message board post.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BoardPostInfo {
    pub id: String,
    pub thread_id: String,
    pub author: String,
    pub content: String,
    pub created_at: String,
}

/// Real-time board notification events.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum BoardNotification {
    ChannelCreated {
        channel: String,
    },
    PostAppended {
        channel: String,
        thread_id: String,
        post_id: String,
        author: String,
    },
}

#[derive(Default)]
struct BoardState {
    channels: HashMap<String, ChannelInfo>,
    threads: HashMap<String, BoardThreadSummary>,
    posts: HashMap<String, Vec<BoardPostInfo>>,
}

/// Agent message board client supporting channels, threads, and notification broadcasting.
#[derive(Clone)]
pub struct RemoteAgentMessageBoard {
    state: Arc<Mutex<BoardState>>,
    notifications: Arc<broadcast::Sender<BoardNotification>>,
}

impl Default for RemoteAgentMessageBoard {
    fn default() -> Self {
        Self::new()
    }
}

impl RemoteAgentMessageBoard {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self {
            state: Arc::new(Mutex::new(BoardState::default())),
            notifications: Arc::new(tx),
        }
    }

    /// Creates a new named channel on the board.
    pub fn create_channel(&self, name: &str, topic: Option<&str>) -> Result<ChannelInfo> {
        let mut st = self.state.lock().unwrap();
        if st.channels.contains_key(name) {
            anyhow::bail!("channel '{name}' already exists");
        }

        let channel = ChannelInfo {
            name: name.to_string(),
            topic: topic.map(str::to_string),
            created_at: Utc::now().to_rfc3339(),
        };

        st.channels.insert(name.to_string(), channel.clone());
        let _ = self.notifications.send(BoardNotification::ChannelCreated {
            channel: name.to_string(),
        });

        Ok(channel)
    }

    /// Lists all existing channels on the message board.
    pub fn list_channels(&self) -> Result<Vec<ChannelInfo>> {
        let st = self.state.lock().unwrap();
        let mut list: Vec<ChannelInfo> = st.channels.values().cloned().collect();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(list)
    }

    /// Posts a message, either creating a new thread or appending to an existing one.
    pub fn post_message(
        &self,
        channel: &str,
        thread_id: Option<&str>,
        author: &str,
        title: Option<&str>,
        content: &str,
    ) -> Result<BoardPostInfo> {
        let mut st = self.state.lock().unwrap();
        if !st.channels.contains_key(channel) {
            anyhow::bail!("channel '{channel}' does not exist");
        }

        let actual_thread_id = match thread_id {
            Some(tid) => {
                if !st.threads.contains_key(tid) {
                    anyhow::bail!("thread '{tid}' not found");
                }
                tid.to_string()
            }
            None => {
                let new_tid = format!("th_{}", Uuid::new_v4().simple());
                let thread_title = title.unwrap_or("Untitled Thread");
                st.threads.insert(
                    new_tid.clone(),
                    BoardThreadSummary {
                        id: new_tid.clone(),
                        channel: channel.to_string(),
                        title: thread_title.to_string(),
                        author: author.to_string(),
                        created_at: Utc::now().to_rfc3339(),
                        post_count: 0,
                    },
                );
                new_tid
            }
        };

        let post_id = format!("post_{}", Uuid::new_v4().simple());
        let post = BoardPostInfo {
            id: post_id.clone(),
            thread_id: actual_thread_id.clone(),
            author: author.to_string(),
            content: content.to_string(),
            created_at: Utc::now().to_rfc3339(),
        };

        st.posts
            .entry(actual_thread_id.clone())
            .or_default()
            .push(post.clone());

        if let Some(t) = st.threads.get_mut(&actual_thread_id) {
            t.post_count += 1;
        }

        let _ = self.notifications.send(BoardNotification::PostAppended {
            channel: channel.to_string(),
            thread_id: actual_thread_id,
            post_id,
            author: author.to_string(),
        });

        Ok(post)
    }

    /// Reads all posts in a specific thread.
    pub fn read_thread(&self, thread_id: &str) -> Result<Vec<BoardPostInfo>> {
        let st = self.state.lock().unwrap();
        let posts = st
            .posts
            .get(thread_id)
            .cloned()
            .context(format!("thread '{thread_id}' not found"))?;
        Ok(posts)
    }

    /// Subscribes to real-time notification events.
    pub fn subscribe(&self) -> broadcast::Receiver<BoardNotification> {
        self.notifications.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_message_board_channel_and_post_lifecycle() {
        let board = RemoteAgentMessageBoard::new();
        let mut rx = board.subscribe();

        // 1. Create channel
        let ch = board
            .create_channel("general", Some("Discussions"))
            .unwrap();
        assert_eq!(ch.name, "general");

        let notif1 = rx.recv().await.unwrap();
        assert_eq!(
            notif1,
            BoardNotification::ChannelCreated {
                channel: "general".to_string()
            }
        );

        // 2. Post new thread
        let post1 = board
            .post_message(
                "general",
                None,
                "coder",
                Some("Architecture RFC"),
                "Let's align on UDS.",
            )
            .unwrap();
        assert_eq!(post1.author, "coder");

        let notif2 = rx.recv().await.unwrap();
        match notif2 {
            BoardNotification::PostAppended {
                channel, author, ..
            } => {
                assert_eq!(channel, "general");
                assert_eq!(author, "coder");
            }
            _ => panic!("Expected PostAppended notification"),
        }

        // 3. Append to thread
        let post2 = board
            .post_message(
                "general",
                Some(&post1.thread_id),
                "reviewer",
                None,
                "Looks solid to me.",
            )
            .unwrap();
        assert_eq!(post2.thread_id, post1.thread_id);

        // 4. Read full thread
        let thread_posts = board.read_thread(&post1.thread_id).unwrap();
        assert_eq!(thread_posts.len(), 2);
        assert_eq!(thread_posts[0].content, "Let's align on UDS.");
        assert_eq!(thread_posts[1].content, "Looks solid to me.");
    }

    #[test]
    fn test_duplicate_channel_error() {
        let board = RemoteAgentMessageBoard::new();
        board.create_channel("tech", None).unwrap();
        assert!(board.create_channel("tech", None).is_err());
    }
}
