# mdminecraft harness client

`mdm_harness_client.py` is a tiny NDJSON client for driving a running `mdminecraft` automation
server from a request script (stdin or file).

## Example (TCP)

```bash
python3 tools/harness_client/mdm_harness_client.py --tcp 127.0.0.1:4242 --auto-id <<'EOF'
{"op":"get_state"}
{"op":"step","ticks":10}
{"op":"screenshot","tag":"overlook"}
{"op":"shutdown"}
EOF
```

## Example (UDS, unix only)

```bash
python3 tools/harness_client/mdm_harness_client.py --uds /tmp/mdm.sock --auto-id <<'EOF'
{"op":"get_state"}
{"op":"shutdown"}
EOF
```

Notes:
- The client sends `hello` automatically (use `--quiet-hello` to suppress printing the hello event).
- Requests run sequentially; `step` may stream intermediate `screenshot` events before `stepped`.
