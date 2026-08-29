#!/bin/bash
TOKEN=$(curl -s -X POST http://localhost:8080/api/v1/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"admin123"}' \
  | python3 -c 'import sys,json; print(json.load(sys.stdin).get("token",""))')
echo "TOKEN: ${TOKEN:0:30}..."
echo "=== Devices ==="
curl -s http://localhost:8080/api/v1/devices -H "Authorization: Bearer $TOKEN" | python3 -m json.tool 2>/dev/null || echo "JSON parse failed"