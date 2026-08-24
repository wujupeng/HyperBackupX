package device

import (
	"fmt"
	"sync"
	"testing"
	"time"

	"github.com/google/uuid"
)

// PERF-006: Control Plane 支持 ≥10000 设备，≥1000 并发上报
func TestPerf006_ControlPlane_10000_Devices_1000_Concurrent(t *testing.T) {
	mgr := NewManager()

	const totalDevices = 10_000
	const concurrentReports = 1_000

	deviceIDs := make([]uuid.UUID, 0, totalDevices)

	registerStart := time.Now()
	for i := 0; i < totalDevices; i++ {
		hostname := fmt.Sprintf("host-%05d", i)
		d, err := mgr.Register(hostname, "windows", "0.1.0", "modern", fmt.Sprintf("fp-%d", i))
		if err != nil {
			t.Fatalf("register device %d failed: %v", i, err)
		}
		deviceIDs = append(deviceIDs, d.ID)
	}
	registerElapsed := time.Since(registerStart)

	allDevices := mgr.List()
	if len(allDevices) != totalDevices {
		t.Fatalf("PERF-006: expected %d devices, got %d", totalDevices, len(allDevices))
	}

	heartbeatStart := time.Now()
	var wg sync.WaitGroup
	wg.Add(concurrentReports)

	for i := 0; i < concurrentReports; i++ {
		go func(idx int) {
			defer wg.Done()
			deviceID := deviceIDs[idx%totalDevices]
			for j := 0; j < 10; j++ {
				mgr.Heartbeat(deviceID)
			}
		}(i)
	}
	wg.Wait()
	heartbeatElapsed := time.Since(heartbeatStart)

	sampleID := deviceIDs[0]
	d, ok := mgr.Get(sampleID)
	if !ok {
		t.Fatalf("PERF-006: device %s not found", sampleID)
	}
	if d.Status != StatusOnline {
		t.Fatalf("PERF-006: device should be online, got %s", d.Status)
	}

	deregisterStart := time.Now()
	half := totalDevices / 2
	for i := 0; i < half; i++ {
		mgr.Deregister(deviceIDs[i])
	}
	deregisterElapsed := time.Since(deregisterStart)

	remaining := mgr.List()
	if len(remaining) != half {
		t.Fatalf("PERF-006: expected %d remaining devices, got %d", half, len(remaining))
	}

	t.Logf("PERF-006 PASSED: %d devices registered in %v, %d concurrent heartbeats in %v, %d deregistered in %v",
		totalDevices, registerElapsed,
		concurrentReports, heartbeatElapsed,
		half, deregisterElapsed)
}

// PERF-006b: 并发注册 + 心跳混合压测
func TestPerf006b_Concurrent_Register_Heartbeat(t *testing.T) {
	mgr := NewManager()

	const concurrentWorkers = 500
	const opsPerWorker = 20

	var wg sync.WaitGroup
	wg.Add(concurrentWorkers)

	start := time.Now()

	for i := 0; i < concurrentWorkers; i++ {
		go func(workerID int) {
			defer wg.Done()
			for j := 0; j < opsPerWorker; j++ {
				hostname := fmt.Sprintf("perf-host-%d-%d", workerID, j)
				d, err := mgr.Register(hostname, "linux", "0.1.0", "standard", hostname)
				if err != nil {
					t.Errorf("register failed: %v", err)
					return
				}
				mgr.Heartbeat(d.ID)
			}
		}(i)
	}
	wg.Wait()

	elapsed := time.Since(start)
	totalOps := concurrentWorkers * opsPerWorker
	allDevices := mgr.List()

	if len(allDevices) != totalOps {
		t.Fatalf("PERF-006b: expected %d devices, got %d", totalOps, len(allDevices))
	}

	t.Logf("PERF-006b PASSED: %d concurrent register+heartbeat ops in %v (%.0f ops/sec)",
		totalOps, elapsed, float64(totalOps)/elapsed.Seconds())
}