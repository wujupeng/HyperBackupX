#!/bin/bash
TOKEN=$(curl -s http://localhost:8080/api/v1/auth/login -H 'Content-Type: application/json' -d @/tmp/login.json | python3 -c "import sys,json; print(json.load(sys.stdin)['token'])")
echo "=== Compat Repos ==="
curl -s http://localhost:8080/api/v1/compat/repositories -H "Authorization: Bearer $TOKEN" | head -c 200
echo
echo "=== Matrix ==="
curl -s http://localhost:8080/api/v1/compat/matrix -H "Authorization: Bearer $TOKEN" | head -c 200
echo
echo "=== Fuzz Report ==="
curl -s http://localhost:8080/api/v1/compat/fuzz/report -H "Authorization: Bearer $TOKEN" | head -c 200
echo
echo "=== Chaos Report ==="
curl -s http://localhost:8080/api/v1/compat/chaos/report -H "Authorization: Bearer $TOKEN" | head -c 200
echo
echo "=== Acceptance ==="
curl -s http://localhost:8080/api/v1/compat/acceptance -H "Authorization: Bearer $TOKEN" | head -c 500
echo
echo "=== Reports ==="
curl -s http://localhost:8080/api/v1/compat/reports -H "Authorization: Bearer $TOKEN" | head -c 200
echo
echo "=== ALL DONE ==="