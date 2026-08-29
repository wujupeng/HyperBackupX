#!/bin/bash
# Generate JWT token and start hbx-agent
export PATH=$HOME/.cargo/bin:$PATH
export HBX_AGENT_CP_URL=http://127.0.0.1:8080
export HBX_AGENT_BADOU_GRPC=http://127.0.0.1:9090
export HBX_BADOU_JWT=$(python3 /tmp/gen_jwt.py)
exec /home/debian/HyperBackupX/target/release/hbx-agent