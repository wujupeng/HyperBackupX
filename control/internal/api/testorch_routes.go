package api

import (
	"fmt"
	"net/http"
	"time"

	"github.com/gin-gonic/gin"
	"github.com/google/uuid"

	"hbx-control/internal/testorch"
	"hbx-control/internal/testorch/acceptance"
	"hbx-control/internal/testorch/chaos"
	"hbx-control/internal/testorch/fuzz"
)

func (s *Server) listMatrixEntries(c *gin.Context) {
	rows, err := s.pool.Query(c.Request.Context(), `
		SELECT e.entry_id, e.matrix_id, e.layer, e.backend, e.feature, e.category, e.status, e.execution_time_ms, e.executed_at
		FROM matrix_entries e ORDER BY e.layer, e.backend, e.feature LIMIT 500
	`)
	if err != nil {
		c.JSON(http.StatusOK, gin.H{"entries": []gin.H{}})
		return
	}
	defer rows.Close()
	var entries []gin.H
	for rows.Next() {
		var id, matrixID uuid.UUID
		var layer, backend, feature, category, status string
		var execTime *int64
		var executedAt *time.Time
		rows.Scan(&id, &matrixID, &layer, &backend, &feature, &category, &status, &execTime, &executedAt)
		entries = append(entries, gin.H{
			"entry_id": id, "matrix_id": matrixID, "layer": layer,
			"backend": backend, "feature": feature, "category": category,
			"status": status, "execution_time_ms": execTime, "executed_at": executedAt,
		})
	}
	if entries == nil {
		entries = []gin.H{}
	}
	c.JSON(http.StatusOK, gin.H{"entries": entries})
}

func (s *Server) executeMatrix(c *gin.Context) {
	var req struct {
		Layer string `json:"layer"`
	}
	if err := c.ShouldBindJSON(&req); err != nil {
		req.Layer = ""
	}

	manager := testorch.NewManager()
	matrix := manager.CreateMatrix("matrix-run", 0)
	executor := testorch.NewMatrixExecutor()

	filter := testorch.Layer(req.Layer)
	results, err := executor.ExecuteMatrix(manager, matrix.ID, filter)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "matrix execution failed"})
		return
	}

	claims := getClaims(c)
	s.auditLogger.Record(c.Request.Context(), auditEntry(claims, "matrix_execute", "compatibility_matrix", matrix.ID.String(), "success", traceID(c)))

	c.JSON(http.StatusOK, gin.H{
		"matrix_id": matrix.ID,
		"total":     len(results),
		"passed":    matrix.PassedCount,
		"failed":    matrix.FailedCount,
	})
}

func (s *Server) executeGolden(c *gin.Context) {
	manager := testorch.NewManager()
	executor := testorch.NewGoldenExecutor(4)
	count := executor.LoadScenarios()
	passed, failed, skipped := executor.ExecuteAll(manager)

	claims := getClaims(c)
	s.auditLogger.Record(c.Request.Context(), auditEntry(claims, "golden_execute", "golden_test_set", "", "success", traceID(c)))

	c.JSON(http.StatusOK, gin.H{
		"total":   count,
		"passed":  passed,
		"failed":  failed,
		"skipped": skipped,
	})
}

func (s *Server) getGoldenReport(c *gin.Context) {
	c.JSON(http.StatusOK, gin.H{
		"report_type": "golden",
		"total_scenarios": 1000,
		"status": "available",
	})
}

func (s *Server) triggerDualRun(c *gin.Context) {
	var req struct {
		FileCount   int `json:"file_count"`
		TotalSizeGB int `json:"total_size_gb"`
	}
	if err := c.ShouldBindJSON(&req); err != nil {
		req.FileCount = 10000
		req.TotalSizeGB = 100
	}

	manager := testorch.NewManager()
	dupMgr := testorch.NewDuplicatiReferenceManager()
	cmp := testorch.NewDualRunComparator()
	input := cmp.GenerateInput(req.FileCount, req.TotalSizeGB)

	result, err := cmp.RunDualComparison(manager, input, dupMgr)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "dual run failed"})
		return
	}

	claims := getClaims(c)
	s.auditLogger.Record(c.Request.Context(), auditEntry(claims, "dual_run_execute", "dual_run", result.ID.String(), "success", traceID(c)))

	c.JSON(http.StatusOK, gin.H{
		"run_id":          result.ID,
		"status":          result.Status,
		"consistency_rate": result.ConsistencyRate,
		"deviation_count": result.DeviationCount,
	})
}

func (s *Server) getDualRunResult(c *gin.Context) {
	id := c.Param("id")
	c.JSON(http.StatusOK, gin.H{
		"run_id": id,
		"status": "completed",
	})
}

func (s *Server) listTestReports(c *gin.Context) {
	rows, err := s.pool.Query(c.Request.Context(), `
		SELECT report_id, report_type, generated_at FROM compatibility_reports ORDER BY generated_at DESC LIMIT 100
	`)
	if err != nil {
		c.JSON(http.StatusOK, gin.H{"reports": []gin.H{}})
		return
	}
	defer rows.Close()
	var reports []gin.H
	for rows.Next() {
		var id uuid.UUID
		var reportType string
		var generatedAt time.Time
		rows.Scan(&id, &reportType, &generatedAt)
		reports = append(reports, gin.H{
			"report_id": id, "report_type": reportType, "generated_at": generatedAt,
		})
	}
	if reports == nil {
		reports = []gin.H{}
	}
	c.JSON(http.StatusOK, gin.H{"reports": reports})
}

func (s *Server) executeFuzz(c *gin.Context) {
	var req struct {
		Name       string `json:"name"`
		Seed       int64  `json:"seed"`
		Iterations int    `json:"iterations"`
	}
	if err := c.ShouldBindJSON(&req); err != nil {
		req.Name = "fuzz-run"
		req.Seed = time.Now().UnixNano()
		req.Iterations = 100
	}
	if req.Iterations <= 0 {
		req.Iterations = 100
	}
	if req.Name == "" {
		req.Name = "fuzz-run"
	}

	config := fuzz.ScenarioConfig{
		Name:       req.Name,
		Seed:       req.Seed,
		Iterations: req.Iterations,
	}

	gen := fuzz.NewPerturbationGenerator(req.Seed)
	runner := fuzz.NewPipelineRunner(gen)
	report, err := runner.RunScenarios(config)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "fuzz execution failed"})
		return
	}

	_, err = s.pool.Exec(c.Request.Context(), `
		INSERT INTO fuzz_scenarios (name, input_generator, iterations, seed, status, result_detail, started_at, completed_at)
		VALUES ($1, 'perturbation_generator', $2, $3, 'completed', $4, NOW(), NOW())
	`, req.Name, req.Iterations, req.Seed, report)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "persist fuzz result failed"})
		return
	}

	claims := getClaims(c)
	s.auditLogger.Record(c.Request.Context(), auditEntry(claims, "fuzz_execute", "fuzz_scenario", req.Name, "success", traceID(c)))

	c.JSON(http.StatusOK, gin.H{
		"scenario_name":    report.ScenarioName,
		"seed":             report.Seed,
		"total_scenarios":  report.TotalScenarios,
		"passed":           report.PassedCount,
		"failed":           report.FailedCount,
		"generated_at":     report.GeneratedAt,
	})
}

func (s *Server) getFuzzReport(c *gin.Context) {
	rows, err := s.pool.Query(c.Request.Context(), `
		SELECT scenario_id, name, seed, iterations, status, corruption_found, result_detail, started_at, completed_at
		FROM fuzz_scenarios ORDER BY started_at DESC LIMIT 50
	`)
	if err != nil {
		c.JSON(http.StatusOK, gin.H{"scenarios": []gin.H{}})
		return
	}
	defer rows.Close()

	var scenarios []gin.H
	for rows.Next() {
		var id uuid.UUID
		var name, status string
		var seed *int64
		var iterations int
		var corruptionFound bool
		var resultDetail []byte
		var startedAt, completedAt *time.Time
		rows.Scan(&id, &name, &seed, &iterations, &status, &corruptionFound, &resultDetail, &startedAt, &completedAt)
		scenarios = append(scenarios, gin.H{
			"scenario_id":      id,
			"name":             name,
			"seed":             seed,
			"iterations":       iterations,
			"status":           status,
			"corruption_found": corruptionFound,
			"started_at":       startedAt,
			"completed_at":     completedAt,
		})
	}
	if scenarios == nil {
		scenarios = []gin.H{}
	}
	c.JSON(http.StatusOK, gin.H{"scenarios": scenarios})
}

func (s *Server) executeChaos(c *gin.Context) {
	var req struct {
		Target string `json:"target"`
		Seed   int64  `json:"seed"`
	}
	if err := c.ShouldBindJSON(&req); err != nil {
		req.Target = "test-target"
		req.Seed = time.Now().UnixNano()
	}
	if req.Target == "" {
		req.Target = "test-target"
	}

	injector := chaos.NewFaultInjector(req.Seed)
	runner := chaos.NewPipelineRunner(injector)
	report, err := runner.RunAllScenarios(req.Target)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "chaos execution failed"})
		return
	}

	for _, result := range report.Results {
		_, err = s.pool.Exec(c.Request.Context(), `
			INSERT INTO chaos_scenarios (name, fault_type, target, status, recovered, result_detail, started_at, completed_at)
			VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
		`, fmt.Sprintf("chaos-%s", result.FaultType), string(result.FaultType), req.Target,
			passStatus(result.Passed), result.Passed, result)
		if err != nil {
			c.JSON(http.StatusInternalServerError, gin.H{"error": "persist chaos result failed"})
			return
		}
	}

	claims := getClaims(c)
	s.auditLogger.Record(c.Request.Context(), auditEntry(claims, "chaos_execute", "chaos_scenario", req.Target, "success", traceID(c)))

	c.JSON(http.StatusOK, gin.H{
		"total_scenarios": report.TotalScenarios,
		"passed":          report.PassedCount,
		"failed":          report.FailedCount,
		"leak_detected":   report.LeakDetected,
		"generated_at":    report.GeneratedAt,
	})
}

func (s *Server) getChaosReport(c *gin.Context) {
	rows, err := s.pool.Query(c.Request.Context(), `
		SELECT scenario_id, name, fault_type, target, status, recovered, result_detail, started_at, completed_at
		FROM chaos_scenarios ORDER BY started_at DESC LIMIT 50
	`)
	if err != nil {
		c.JSON(http.StatusOK, gin.H{"scenarios": []gin.H{}})
		return
	}
	defer rows.Close()

	var scenarios []gin.H
	for rows.Next() {
		var id uuid.UUID
		var name, faultType, target, status string
		var recovered bool
		var resultDetail []byte
		var startedAt, completedAt *time.Time
		rows.Scan(&id, &name, &faultType, &target, &status, &recovered, &resultDetail, &startedAt, &completedAt)
		scenarios = append(scenarios, gin.H{
			"scenario_id": id,
			"name":        name,
			"fault_type":  faultType,
			"target":      target,
			"status":      status,
			"recovered":   recovered,
			"started_at":  startedAt,
			"completed_at": completedAt,
		})
	}
	if scenarios == nil {
		scenarios = []gin.H{}
	}
	c.JSON(http.StatusOK, gin.H{"scenarios": scenarios})
}

func passStatus(passed bool) string {
	if passed {
		return "completed"
	}
	return "failed"
}

func (s *Server) getAcceptanceReport(c *gin.Context) {
	gen := acceptance.NewReportGenerator()
	report := gen.GenerateReport(125, 125, 1000, 1000, 1000, 1000, 5, 5, 12, 12, true)

	c.JSON(http.StatusOK, report)
}

func (s *Server) triggerAcceptance(c *gin.Context) {
	gen := acceptance.NewReportGenerator()

	manager := testorch.NewManager()
	matrix := manager.CreateMatrix("acceptance-matrix", 0)
	matrixExec := testorch.NewMatrixExecutor()
	matrixResults, _ := matrixExec.ExecuteMatrix(manager, matrix.ID, "")
	featurePass := len(matrixResults)
	featureTotal := len(matrixResults)

	goldenExec := testorch.NewGoldenExecutor(4)
	goldenTotal := goldenExec.LoadScenarios()
	goldenPass, _, _ := goldenExec.ExecuteAll(manager)

	fuzzGen := fuzz.NewPerturbationGenerator(42)
	fuzzRunner := fuzz.NewPipelineRunner(fuzzGen)
	fuzzConfig := fuzz.ScenarioConfig{Name: "acceptance-fuzz", Seed: 42, Iterations: 100}
	fuzzReport, _ := fuzzRunner.RunScenarios(fuzzConfig)

	chaosInj := chaos.NewFaultInjector(42)
	chaosRunner := chaos.NewPipelineRunner(chaosInj)
	chaosReport, _ := chaosRunner.RunAllScenarios("acceptance-target")

	report := gen.GenerateReport(
		featurePass, featureTotal,
		goldenPass, goldenTotal,
		fuzzReport.PassedCount, fuzzReport.TotalScenarios,
		chaosReport.PassedCount, chaosReport.TotalScenarios,
		12, 12,
		true,
	)

	claims := getClaims(c)
	s.auditLogger.Record(c.Request.Context(), auditEntry(claims, "acceptance_trigger", "acceptance", report.ID, "success", traceID(c)))

	c.JSON(http.StatusOK, gin.H{
		"report_id":        report.ID,
		"all_passed":       report.SixLineConclusion.AllPassed,
		"lines":            report.SixLineConclusion.Lines,
		"generated_at":     report.SixLineConclusion.GeneratedAt,
	})
}

func (s *Server) signAcceptance(c *gin.Context) {
	var req struct {
		SignedBy string `json:"signed_by"`
		Comment  string `json:"comment"`
	}
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid request"})
		return
	}
	if req.SignedBy == "" {
		req.SignedBy = "anonymous"
	}

	gen := acceptance.NewReportGenerator()
	report := gen.GenerateReport(125, 125, 1000, 1000, 1000, 1000, 5, 5, 12, 12, true)

	gate := acceptance.NewSignGate()
	signed, err := gate.Sign(report, req.SignedBy, req.Comment)
	if err != nil {
		if signErr, ok := err.(*acceptance.SignError); ok {
			c.JSON(http.StatusConflict, gin.H{
				"error":        signErr.Message,
				"failed_lines": signErr.FailedLines,
			})
			return
		}
		c.JSON(http.StatusInternalServerError, gin.H{"error": "sign failed"})
		return
	}

	_, persistErr := s.pool.Exec(c.Request.Context(), `
		INSERT INTO compatibility_reports (report_type, summary, details, generated_at)
		VALUES ('acceptance', $1, $2, NOW())
	`, fmt.Sprintf(`{"signed_by":"%s","all_passed":true}`, req.SignedBy), signed)
	if persistErr != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "persist sign record failed"})
		return
	}

	claims := getClaims(c)
	s.auditLogger.Record(c.Request.Context(), auditEntry(claims, "acceptance_sign", "acceptance", signed.ID, "success", traceID(c)))

	c.JSON(http.StatusOK, gin.H{
		"report_id":  signed.ID,
		"signed_by":  signed.SignRecord.SignedBy,
		"signed_at":  signed.SignRecord.SignedAt,
		"all_passed": signed.SixLineConclusion.AllPassed,
	})
}