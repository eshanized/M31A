package execute

// execute_view.go — view helpers for the Execute screen.
// The main View() method is in execute_model.go.

// renderTaskSpinner renders the current spinner frame for a running task.
func renderTaskSpinner(em *ExecuteModel) string {
	if em == nil {
		return ""
	}
	return em.theme.Spinner.Render(em.spinner.Peek())
}
