package codeintel

import (
	"testing"
)

func TestTreeSitterParser_CanParse(t *testing.T) {
	p := &TreeSitterParser{}
	cases := map[string]bool{
		"main.go": true, "app.ts": true,
		"util.js": true, "main.py": true, "main.rs": true,
		"app.tsx": false, "README.md": false, "style.css": false,
	}
	for path, want := range cases {
		if got := p.CanParse(path); got != want {
			t.Errorf("CanParse(%q) = %v, want %v", path, got, want)
		}
	}
}

func TestTreeSitterParser_ParseGo(t *testing.T) {
	p := &TreeSitterParser{}
	src := []byte(`package main

import (
	"fmt"
	"strings"
	"github.com/eshanized/M31A/internal/types"
)

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

const MaxSize = 100
var DefaultName = "test"
`)
	info, err := p.Parse("main.go", src)
	if err != nil {
		t.Fatal(err)
	}

	if info.Language != "go" {
		t.Errorf("Language = %q, want %q", info.Language, "go")
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

	// Check functions
	funcNames := make(map[string]bool)
	for _, f := range info.Funcs {
		funcNames[f.Name] = true
	}
	for _, want := range []string{"Exported", "unexported", "Run"} {
		if !funcNames[want] {
			t.Errorf("missing function: %s", want)
		}
	}

	// Check types
	typeNames := make(map[string]bool)
	for _, ti := range info.Types {
		typeNames[ti.Name] = true
	}
	if !typeNames["Engine"] {
		t.Error("missing Engine type")
	}

	// Check exports
	exportNames := make(map[string]bool)
	for _, s := range info.Exports {
		exportNames[s.Name] = true
	}
	for _, want := range []string{"Exported", "Engine", "MaxSize", "DefaultName"} {
		if !exportNames[want] {
			t.Errorf("missing export: %s", want)
		}
	}
}

func TestTreeSitterParser_ParseTypeScript(t *testing.T) {
	p := &TreeSitterParser{}
	src := []byte(`
import { useState } from 'react';
import type { Config } from './config';
import * as utils from '../utils';
import './styles.css';
import express from 'express';

export function greet(name: string): string {
  return "hello " + name;
}

export class UserService {
  getUser(id: string) {}
}

export interface Config {
  port: number;
  host: string;
}

export enum Status {
  Active,
  Inactive,
}
`)
	info, err := p.Parse("app.ts", src)
	if err != nil {
		t.Fatal(err)
	}

	if len(info.Imports) < 4 {
		t.Fatalf("expected at least 4 imports, got %d", len(info.Imports))
	}

	if len(info.Funcs) < 1 {
		t.Errorf("expected at least 1 function, got %d", len(info.Funcs))
	}

	typeNames := make(map[string]bool)
	for _, ti := range info.Types {
		typeNames[ti.Name] = true
	}
	for _, want := range []string{"UserService", "Config", "Status"} {
		if !typeNames[want] {
			t.Errorf("missing type: %s", want)
		}
	}
}

func TestTreeSitterParser_ParsePython(t *testing.T) {
	p := &TreeSitterParser{}
	src := []byte(`
import os
import sys
from pathlib import Path
from typing import List, Optional
from .utils import helper

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

	classNames := make(map[string]bool)
	for _, ti := range info.Types {
		classNames[ti.Name] = true
	}
	if !classNames["UserService"] {
		t.Error("missing UserService class")
	}

	funcNames := make(map[string]bool)
	for _, f := range info.Funcs {
		funcNames[f.Name] = true
	}
	for _, want := range []string{"process", "fetch", "_private"} {
		if !funcNames[want] {
			t.Errorf("missing function: %s", want)
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
		"util.js":   "javascript",
		"main.py":   "python",
		"main.rs":   "rust",
		"app.tsx":   "",
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
		// For tree-sitter parsed files, check the FileInfo language instead
		info, _ := p.Parse(path, []byte("// test"))
		if info != nil && info.Language != wantLang {
			t.Errorf("ParserForFile(%q).Parse().Language = %q, want %q", path, info.Language, wantLang)
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
		if got := isExported(tc.name, tc.lang); got != tc.exported {
			t.Errorf("isExported(%q, %q) = %v, want %v", tc.name, tc.lang, got, tc.exported)
		}
	}
}

func TestSymbolTrie(t *testing.T) {
	trie := NewSymbolTrie()
	trie.Insert("GetUser")
	trie.Insert("GetUserByID")
	trie.Insert("SetUser")
	trie.Insert("UserService")

	if !trie.Search("GetUser") {
		t.Error("expected Search GetUser to return true")
	}
	if trie.Search("Missing") {
		t.Error("expected Search Missing to return false")
	}

	if !trie.HasPrefix("Get") {
		t.Error("expected HasPrefix Get to return true")
	}
	if trie.HasPrefix("Zzz") {
		t.Error("expected HasPrefix Zzz to return false")
	}

	matches := trie.PrefixSearch("Get")
	if len(matches) != 2 {
		t.Errorf("PrefixSearch Get: got %d matches, want 2", len(matches))
	}

	matches = trie.PrefixSearch("Set")
	if len(matches) != 1 {
		t.Errorf("PrefixSearch Set: got %d matches, want 1", len(matches))
	}
}
