# MCP Server Example

This is a basic example of an MCP (Model Context Protocol) server used as a M31A
extension. It demonstrates how to expose tools and prompts to the M31A.

## Description

The contents of this directory are a valid MCP server implementation using the
`@modelcontextprotocol/sdk`. It exposes:

- A tool `fetch_posts` that mock-fetches posts.
- A prompt `poem-writer`.

## Structure

- `example.js`: The main server entry point.
- `m31a-extension.json`: The configuration file that tells M31A how to use this
  extension.
- `package.json`: Helper for dependencies.

## How to Use

1.  Navigate to this directory:

    ```bash
    cd packages/cli/src/commands/extensions/examples/mcp-server
    ```

2.  Install dependencies:
    ```bash
    npm install
    ```

This example is typically used by `m31a extensions new`.
