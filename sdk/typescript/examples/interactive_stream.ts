/**
 * Example: Interactive streaming with reasoning chunks and tool observation.
 */

import { M31AClient } from '../src/index.js';

async function main() {
  const client = new M31AClient();
  await client.connect();

  console.log('Connected to M31A App Server.');
  const diagnostics = await client.getDiagnostics();
  console.log(`Active Model: ${diagnostics.active_model}`);

  const thread = await client.createThread({ cwd: process.cwd() });
  console.log(`Created Thread: ${thread.id}`);

  // Run streaming turn with reasoning tokens and tool tracking
  const turn = await thread.run('Explain why Rust memory safety prevents data races at compile time.', {
    onReasoning: (chunk) => {
      process.stderr.write(`\x1b[35m${chunk}\x1b[0m`); // Magenta reasoning path
    },
    onContent: (chunk) => {
      process.stdout.write(chunk);
    },
    onToolStarted: (ev) => {
      console.log(`\n\x1b[33m[Tool Invoked: ${ev.tool}]\x1b[0m args: ${ev.arguments}`);
    },
    onToolFinished: (ev) => {
      console.log(`\x1b[32m[Tool Finished: ${ev.call_id}]\x1b[0m success=${ev.success}`);
    },
  });

  const finalAnswer = await turn.result();
  console.log('\n\n--- Final Assistant Output ---\n', finalAnswer);

  await client.close();
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch(console.error);
}
