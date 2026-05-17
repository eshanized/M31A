package codeintel

import (
	"testing"
)

func TestGoParser_CanParse(t *testing.T) {
	p := &GoParser{}
	if !p.CanParse("main.go") {
		t.Error("expected CanParse for .go file")
	}
	if p.CanParse("main.py") {
		t.Error("unexpected CanParse for .py file")
	}
}

func TestGoParser_ParseImports(t *testing.T) {
	p := &GoParser{}
	src := []byte(`package main

import (
	"fmt"
	"strings"
	"github.com/eshanized/M31A/internal/types"
)

func main() {
	fmt.Println("hello")
}
`)
	info, err := p.Parse("main.go", src)
	if err != nil {
		t.Fatal(err)
	}
	if len(info.Imports) != 3 {
		t.Fatalf("expected 3 imports, got %d", len(info.Imports))
	}
	paths := make(map[string]bool)
	for _, imp := range info.Imports {
		paths[imp.Path] = true
	}
	for _, want := range []string{"fmt", "strings", "github.com/eshanized/M31A/internal/types"} {
		if !paths[want] {
			t.Errorf("missing import: %s", want)
		}
	}
}

func TestGoParser_ParseFunctions(t *testing.T) {
	p := &GoParser{}
	src := []byte(`package main

import "fmt"

func Exported() string {
	return "hello"
}

func unexported(x int, y string) (bool, error) {
	return true, nil
}

type Engine struct {
	Name string
}

func (e *Engine) Run(ctx context.Context) error {
	return nil
}
`)
	info, err := p.Parse("engine.go", src)
	if err != nil {
		t.Fatal(err)
	}

	if len(info.Funcs) != 3 {
		t.Fatalf("expected 3 functions, got %d", len(info.Funcs))
	}

	found := make(map[string]FuncSignature)
	for _, f := range info.Funcs {
		found[f.Name] = f
	}

	exp, ok := found["Exported"]
	if !ok {
		t.Fatal("missing Exported function")
	}
	if !exp.Exported {
		t.Error("Exported should be marked exported")
	}
	if exp.Returns != "string" {
		t.Errorf("Exported returns: got %q, want %q", exp.Returns, "string")
	}

	run, ok := found["Run"]
	if !ok {
		t.Fatal("missing Run method")
	}
	if run.Receiver != "*Engine" {
		t.Errorf("Run receiver: got %q, want %q", run.Receiver, "*Engine")
	}
}

func TestGoParser_ParseTypes(t *testing.T) {
	p := &GoParser{}
	src := []byte(`package main

type Config struct {
	Name    string
	Verbose bool
}

type Handler interface {
	Handle(req Request) error
	Close() error
}

type Mode string
`)
	info, err := p.Parse("types.go", src)
	if err != nil {
		t.Fatal(err)
	}

	if len(info.Types) != 3 {
		t.Fatalf("expected 3 types, got %d", len(info.Types))
	}

	found := make(map[string]TypeInfo)
	for _, ti := range info.Types {
		found[ti.Name] = ti
	}

	cfg, ok := found["Config"]
	if !ok {
		t.Fatal("missing Config struct")
	}
	if cfg.Kind != "struct" {
		t.Errorf("Config kind: got %q, want %q", cfg.Kind, "struct")
	}
	if len(cfg.Fields) != 2 {
		t.Errorf("Config fields: got %d, want 2", len(cfg.Fields))
	}

	handler, ok := found["Handler"]
	if !ok {
		t.Fatal("missing Handler interface")
	}
	if handler.Kind != "interface" {
		t.Errorf("Handler kind: got %q, want %q", handler.Kind, "interface")
	}
	if len(handler.Methods) != 2 {
		t.Errorf("Handler methods: got %d, want 2", len(handler.Methods))
	}
}

func TestGoParser_ParseConstants(t *testing.T) {
	p := &GoParser{}
	src := []byte(`package main

const MaxSize = 100
const (
	StatusOK    = 200
	StatusError = 500
)
var DefaultName = "test"
`)
	info, err := p.Parse("consts.go", src)
	if err != nil {
		t.Fatal(err)
	}

	constCount := 0
	varCount := 0
	for _, s := range info.Exports {
		switch s.Kind {
		case "const":
			constCount++
		case "var":
			varCount++
		}
	}
	if constCount != 3 {
		t.Errorf("expected 3 consts, got %d", constCount)
	}
	if varCount != 1 {
		t.Errorf("expected 1 var, got %d", varCount)
	}
}

func TestTypeScriptParser_CanParse(t *testing.T) {
	p := &TypeScriptParser{}
	cases := map[string]bool{
		"app.ts": true, "app.tsx": true, "util.js": true,
		"module.mjs": true, "config.cjs": true, "main.py": false, "main.go": false,
	}
	for path, want := range cases {
		if got := p.CanParse(path); got != want {
			t.Errorf("CanParse(%q) = %v, want %v", path, got, want)
		}
	}
}

func TestTypeScriptParser_ParseImports(t *testing.T) {
	p := &TypeScriptParser{}
	src := []byte(`
import { useState } from 'react';
import type { Config } from './config';
import * as utils from '../utils';
import './styles.css';
import express from 'express';
`)
	info, err := p.Parse("app.tsx", src)
	if err != nil {
		t.Fatal(err)
	}
	if len(info.Imports) < 4 {
		t.Fatalf("expected at least 4 imports, got %d", len(info.Imports))
	}
	paths := make(map[string]bool)
	for _, imp := range info.Imports {
		paths[imp.Path] = true
	}
	for _, want := range []string{"react", "./config", "../utils", "./styles.css", "express"} {
		if !paths[want] {
			t.Errorf("missing import: %s", want)
		}
	}
}

func TestTypeScriptParser_ParseExports(t *testing.T) {
	p := &TypeScriptParser{}
	src := []byte(`
export function greet(name: string): string {
  return "hello " + name;
}

export const add = (a: number, b: number): number => a + b;

export class UserService {
  getUser(id: string) {}
}

export interface Config {
  port: number;
  host: string;
}

export type Mode = "dev" | "prod";

export enum Status {
  Active,
  Inactive,
}
`)
	info, err := p.Parse("service.ts", src)
	if err != nil {
		t.Fatal(err)
	}

	if len(info.Funcs) < 2 {
		t.Errorf("expected at least 2 functions, got %d", len(info.Funcs))
	}

	typeNames := make(map[string]bool)
	for _, ti := range info.Types {
		typeNames[ti.Name] = true
	}
	for _, want := range []string{"UserService", "Config", "Mode", "Status"} {
		if !typeNames[want] {
			t.Errorf("missing type: %s", want)
		}
	}
}

func TestPythonParser_CanParse(t *testing.T) {
	p := &PythonParser{}
	if !p.CanParse("main.py") {
		t.Error("expected CanParse for .py")
	}
	if p.CanParse("main.go") {
		t.Error("unexpected CanParse for .go")
	}
}

func TestPythonParser_ParseImports(t *testing.T) {
	p := &PythonParser{}
	src := []byte(`
import os
import sys
from pathlib import Path
from typing import List, Optional
from .utils import helper
`)
	info, err := p.Parse("main.py", src)
	if err != nil {
		t.Fatal(err)
	}
	if len(info.Imports) < 4 {
		t.Fatalf("expected at least 4 imports, got %d", len(info.Imports))
	}
	paths := make(map[string]bool)
	for _, imp := range info.Imports {
		paths[imp.Path] = true
	}
	for _, want := range []string{"os", "sys", "pathlib", "typing", ".utils"} {
		if !paths[want] {
			t.Errorf("missing import: %s", want)
		}
	}
}

func TestPythonParser_ParseClassesAndFuncs(t *testing.T) {
	p := &PythonParser{}
	src := []byte(`
class UserService:
    def __init__(self, db):
        self.db = db

    def get_user(self, user_id: str) -> Optional[User]:
        pass

class _Internal:
    pass

def process(data: List[str]) -> bool:
    return True

async def fetch(url: str) -> Response:
    pass

def _private():
    pass
`)
	info, err := p.Parse("service.py", src)
	if err != nil {
		t.Fatal(err)
	}

	classNames := make(map[string]TypeInfo)
	for _, ti := range info.Types {
		classNames[ti.Name] = ti
	}

	if _, ok := classNames["UserService"]; !ok {
		t.Error("missing UserService class")
	}
	if internal, ok := classNames["_Internal"]; ok {
		for _, s := range info.Exports {
			if s.Name == "_Internal" && s.Exported {
				t.Error("_Internal should not be exported")
			}
		}
		_ = internal
	}

	funcNames := make(map[string]FuncSignature)
	for _, f := range info.Funcs {
		funcNames[f.Name] = f
	}

	if _, ok := funcNames["process"]; !ok {
		t.Error("missing process function")
	}
	if _, ok := funcNames["fetch"]; !ok {
		t.Error("missing fetch async function")
	}
	if _, ok := funcNames["_private"]; !ok {
		t.Error("missing _private function")
	}
}

func TestRustParser_CanParse(t *testing.T) {
	p := &RustParser{}
	if !p.CanParse("main.rs") {
		t.Error("expected CanParse for .rs")
	}
	if p.CanParse("main.go") {
		t.Error("unexpected CanParse for .go")
	}
}

func TestRustParser_ParseUseStatements(t *testing.T) {
	p := &RustParser{}
	src := []byte(`
use std::io;
use std::collections::HashMap;
use crate::types::Config;
use serde::Deserialize;
`)
	info, err := p.Parse("main.rs", src)
	if err != nil {
		t.Fatal(err)
	}
	if len(info.Imports) < 4 {
		t.Fatalf("expected at least 4 imports, got %d", len(info.Imports))
	}
	paths := make(map[string]bool)
	for _, imp := range info.Imports {
		paths[imp.Path] = true
	}
	for _, want := range []string{"std::io", "std::collections::HashMap", "crate::types::Config", "serde::Deserialize"} {
		if !paths[want] {
			t.Errorf("missing use: %s", want)
		}
	}
}

func TestRustParser_ParsePubItems(t *testing.T) {
	p := &RustParser{}
	src := []byte(`
pub struct Config {
    pub name: String,
    pub verbose: bool,
}

pub enum Status {
    Active,
    Inactive,
}

pub trait Handler {
    fn handle(&self, req: Request) -> Result<(), Error>;
}

pub fn process(data: &[u8]) -> Result<Vec<u8>, Error> {
    Ok(vec![])
}

pub const MAX_SIZE: usize = 1024;

pub type Callback = Box<dyn Fn(i32) -> bool>;

fn private_func() {}
`)
	info, err := p.Parse("lib.rs", src)
	if err != nil {
		t.Fatal(err)
	}

	exported := make(map[string]SymbolInfo)
	for _, s := range info.Exports {
		exported[s.Name] = s
	}
	for _, want := range []string{"Config", "Status", "Handler", "process", "MAX_SIZE", "Callback"} {
		if _, ok := exported[want]; !ok {
			t.Errorf("missing pub item: %s", want)
		}
	}

	typeNames := make(map[string]TypeInfo)
	for _, ti := range info.Types {
		typeNames[ti.Name] = ti
	}
	if _, ok := typeNames["Config"]; !ok {
		t.Error("missing Config struct type")
	}
	if _, ok := typeNames["Status"]; !ok {
		t.Error("missing Status enum type")
	}
	if _, ok := typeNames["Handler"]; !ok {
		t.Error("missing Handler trait type")
	}
}

func TestParserForFile(t *testing.T) {
	parsers := AllParsers()
	cases := map[string]string{
		"main.go":   "go",
		"app.ts":    "typescript",
		"app.tsx":   "typescript",
		"util.js":   "typescript",
		"main.py":   "python",
		"main.rs":   "rust",
		"README.md": "",
	}
	for path, wantLang := range cases {
		p := ParserForFile(path, parsers)
		if wantLang == "" {
			if p != nil {
				t.Errorf("ParserForFile(%q) = %v, want nil", path, p.Language())
			}
			continue
		}
		if p == nil {
			t.Errorf("ParserForFile(%q) = nil, want %s parser", path, wantLang)
			continue
		}
		if p.Language() != wantLang {
			t.Errorf("ParserForFile(%q).Language() = %q, want %q", path, p.Language(), wantLang)
		}
	}
}

func TestIsExported(t *testing.T) {
	cases := []struct {
		name     string
		lang     string
		exported bool
	}{
		{"Exported", "go", true},
		{"unexported", "go", false},
		{"public_func", "python", true},
		{"_private", "python", false},
		{"__dunder__", "python", false},
		{"myFunc", "typescript", true},
	}
	for _, tc := range cases {
		if got := IsExported(tc.name, tc.lang); got != tc.exported {
			t.Errorf("IsExported(%q, %q) = %v, want %v", tc.name, tc.lang, got, tc.exported)
		}
	}
}
