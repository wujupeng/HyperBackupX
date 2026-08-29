import socket
target = "192.168.2.122"
ports = [3389, 22, 5985, 5986, 445, 135]
for p in ports:
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.settimeout(2)
    try:
        s.connect((target, p))
        print(f"Port {p}: OPEN")
    except Exception as e:
        print(f"Port {p}: CLOSED/filtered ({e})")
    finally:
        s.close()