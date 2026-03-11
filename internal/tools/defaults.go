package tools

import "github.com/eshanized/M31A/internal/config"

func DefaultDispatcher(workDir, backupDir, sessionsDir string, cfg *config.PermissionsConfig) (*Dispatcher, error) {
	d := NewDispatcher(cfg)
	d.workDir_ = workDir
	if err := d.Register(NewBash(workDir)); err != nil {
		return nil, err
	}
	if err := d.Register(NewFileRead(workDir)); err != nil {
		return nil, err
	}
	if err := d.Register(NewFileWrite(workDir, backupDir)); err != nil {
		return nil, err
	}
	if err := d.Register(NewEdit(workDir, backupDir)); err != nil {
		return nil, err
	}
	todo := NewTodoWrite(sessionsDir, "")
	d.todoWrite = todo
	if err := d.Register(todo); err != nil {
		return nil, err
	}
	if err := d.Register(NewWebFetch(sessionsDir, false)); err != nil {
		return nil, err
	}
	if err := d.Register(NewAskUserQuestion(d.questionReqCh, d.questionRespCh, &d.pendingQuestions)); err != nil {
		return nil, err
	}
	if err := d.Register(NewGlob(workDir)); err != nil {
		return nil, err
	}
	if err := d.Register(NewGrep(workDir)); err != nil {
		return nil, err
	}
	if err := d.Register(NewFileList(workDir)); err != nil {
		return nil, err
	}
	if err := d.Register(NewFileDelete(workDir, backupDir)); err != nil {
		return nil, err
	}
	if err := d.Register(NewFileMove(workDir)); err != nil {
		return nil, err
	}
	return d, nil
}
