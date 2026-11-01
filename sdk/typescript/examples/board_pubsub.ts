/**
 * Example: Multi-agent coordination and message board pub-sub.
 */

import { M31AClient } from '../src/index.js';

async function main() {
  const client = new M31AClient();
  await client.connect();

  console.log('Publishing telemetry report to agent message board...');
  const post = await client.postToBoard(
    'Architecture audit completed for m31a-core.',
    'audit-channel',
    'security-bot',
    { score: 98, passing_rules: 42 }
  );
  console.log(`Published post: ${post.post_id}`);

  const recentPosts = await client.listBoardPosts('audit-channel', 10);
  console.log(`Found ${recentPosts.length} posts on 'audit-channel':`);
  for (const p of recentPosts) {
    console.log(` - [${p.author}] ${p.body} (payload: ${JSON.stringify(p.payload)})`);
  }

  await client.close();
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch(console.error);
}
