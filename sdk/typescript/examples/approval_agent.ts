/**
 * Example: Automated tool approval bot and checkpoint rollback.
 */

import { M31AClient, ApprovalRequest, ApprovalDecision } from '../src/index.js';

async function main() {
  const client = new M31AClient();
  await client.connect();

  const thread = await client.createThread();
  console.log(`Initialized Session: ${thread.id}`);

  // Create safety checkpoint before running risky instructions
  const checkpoint = await thread.createCheckpoint('Pre-Mutation Checkpoint', ['src/']);
  console.log(`Created Snapshot Checkpoint: ${checkpoint.id}`);

  // Execute turn with automated approval policy
  const turn = await thread.run('Run build checks and compile test targets', {
    onApprovalRequested: async (req: ApprovalRequest): Promise<ApprovalDecision> => {
      console.log(`[Approval Prompt] Tool: ${req.tool}, Reason: ${req.reason}`);
      if (req.command && req.command.includes('rm -rf')) {
        console.warn('Denied dangerous delete command!');
        return { decision: 'deny', reason: 'Destructive command blocked by client policy' };
      }
      console.log(`Approving command: ${req.command}`);
      return { decision: 'allow' };
    },
    onContent: (chunk) => process.stdout.write(chunk),
  });

  await turn.result();
  await client.close();
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch(console.error);
}
