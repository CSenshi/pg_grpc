# Record with:  vhs docs/demo.tape -o website/static/img/demo.gif
# Prereqs (run once):
#   cargo pgrx install --release
#   cargo pgrx start pg18

Output demo.gif

Set FontSize 16
Set Width 800
Set Height 350
Set Theme "Dracula"
Set TypingSpeed 40ms
Set Padding 20

# Launch psql and reset the extension silently.
Hide
Type "cargo pgrx connect pg18"
Enter
Sleep 2500ms
Type "DROP EXTENSION IF EXISTS pg_grpc;"
Enter
Sleep 600ms
Ctrl+L
Sleep 200ms
Show

# gRPC call with metadata (v0.3 feature)
Type "CREATE EXTENSION pg_grpc;"
Sleep 200ms
Enter
Sleep 200ms
Enter
Sleep 500ms
Type "SELECT grpc_call("
Enter
Type "    'grpcb.in:9000',"
Enter
Type "    'grpcbin.GRPCBin/DummyUnary',"
Enter
Type `    '{"f_string": "Hello from Postgres!"}'::jsonb,`
Enter
Type `    metadata => '{"authorization": "Bearer token123"}'::jsonb`
Enter
Type ");"
Sleep 200ms
Enter

Sleep 4s
