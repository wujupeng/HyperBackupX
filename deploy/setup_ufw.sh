#!/bin/bash
sudo ufw --force enable
sudo ufw allow 3000/tcp comment "HBX Web Dashboard"
sudo ufw allow 8080/tcp comment "HBX Control Plane API"
sudo ufw allow 5432/tcp comment "HBX PostgreSQL"
sudo ufw allow 22/tcp comment "SSH"
sudo ufw status verbose