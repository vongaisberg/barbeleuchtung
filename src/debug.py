import socket
import struct
import time
import threading

# Configuration
BROADCAST_IP = '10.255.255.255'  # Broadcast for 10.0.0.0/8
PORT = 6454
POLL_INTERVAL = 2.5
WATCH_LIST = ['10.0.1.1', '10.0.1.3']

# Art-Net OpCodes
OP_POLL = 0x2000
OP_POLL_REPLY = 0x2100

# Global tracker for the current cycle
received_this_cycle = set()
lock = threading.Lock()

# Use a mutable object to allow modification from multiple threads
time_start = [0.0]

def create_artpoll():
    # Art-Net Header "Art-Net\0"
    header = b'Art-Net\x00'
    # OpCode (Low Byte, High Byte) -> 0x2000
    opcode = struct.pack('<H', OP_POLL) 
    # ProtoVer (14)
    proto_ver = struct.pack('>H', 14)
    # TalkToMe (Diagnostics enabled)
    talk_to_me = b'\x02'
    # Priority
    priority = b'\x00'
    return header + opcode + proto_ver + talk_to_me + priority

def get_universes(data):
    """
    Parses ArtPollReply to find subscribed universes.
    Standard calculation: (Net * 256) + (Sub-Net * 16) + Universe (SwOut)
    """
    try:
        # Byte 18: NetSwitch (7 bits)
        net = data[18] & 0x7F
        # Byte 19: SubSwitch (4 bits)
        sub = data[19] & 0x0F
        
        universes = []
        # Bytes 190-193: Output Sub-Switch for 4 ports
        # Bytes 174-177: Port Types (bit 7 = output enabled)
        for i in range(4):
            port_type = data[174 + i]
            # Check if port is an output (bit 7 set)
            if port_type & 0x80:
                sw_out = data[190 + i] & 0x0F
                univ = (net << 8) | (sub << 4) | sw_out
                universes.append(univ)
        return universes
    except Exception as e:
        return [f"Parse Error: {e}"]

def listener(sock):
    while True:
        try:
            data, addr = sock.recvfrom(1024)
            
            # Basic Header Check
            if not data.startswith(b'Art-Net\x00'):
                continue

            # Check OpCode (bytes 8-9)
            opcode = struct.unpack('<H', data[8:10])[0]

            if opcode == OP_POLL_REPLY:
                sender_ip = addr[0]
                universes = get_universes(data)
                
                # Mark as received
                with lock:
                    received_this_cycle.add(sender_ip)
                    ts = time_start[0]

                print(f"[REPLY] {(time.time() - ts)*1000:.1f}ms   {sender_ip} is active. Universes: {universes}")

        except Exception as e:
            print(f"Socket Error: {e}")

def main():
    # Setup UDP Socket
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.setsockopt(socket.SOL_SOCKET, socket.SO_BROADCAST, 1)
    # Bind to all interfaces
    try:
        sock.bind(('', PORT+1))
    except OSError:
        print("Error: Could not bind to port 6454. Close other ArtNet software first!")
        return

    # Start Listener Thread
    t = threading.Thread(target=listener, args=(sock,), daemon=True)
    t.start()

    print(f"--- ArtNet Debugger Started ---")
    print(f"Broadcasting to {BROADCAST_IP} every {POLL_INTERVAL}s")
    print(f"Watching for: {WATCH_LIST}")
    print("-------------------------------")

    poll_packet = create_artpoll()

    while True:
        # 1. Clear previous cycle
        with lock:
            received_this_cycle.clear()
            time_start[0] = time.time()

        # 2. Send Poll
        print(f"\n---> Sending ArtPoll...")
        sock.sendto(poll_packet, (BROADCAST_IP, PORT))

        # 3. Wait for replies
        time.sleep(POLL_INTERVAL)

        # 4. Check who is missing
        with lock:
            for ip in WATCH_LIST:
                if ip not in received_this_cycle:
                    print(f"\n!!!!!!!! WARNING: {ip} DID NOT REPLY !!!!!!!!")
                    print(f"!!!!!!!! Possible Packet Loss or Timeout !!!!!!!!\n")

if __name__ == "__main__":
    main()
