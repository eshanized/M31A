package efie

import (
	"go/ast"
	"go/parser"
	"go/token"
	"path/filepath"
	"regexp"
	"strings"
	"unicode"
)

// ── Go Parser (go/ast) ─────────────────────────────────────────────────────

type GoParser struct{}

func (p *GoParser) Language() string          { return "go" }
func (p *GoParser) CanParse(path string) bool { return filepath.Ext(path) == ".go" }

func (p *GoParser) Parse(path string, content []byte) (*FileInfo, error) {
	fset := token.NewFileSet()
	f, err := parser.ParseFile(fset, path, content, parser.ParseComments)
	if err != nil {
		return nil, err
	}

	info := &FileInfo{Path: path, Language: "go"}

	for _, imp := range f.Imports {
		impPath := strings.Trim(imp.Path.Value, `"`)
		info.Imports = append(info.Imports, ImportInfo{Path: impPath})
	}

	for _, decl := range f.Decls {
		switch d := decl.(type) {
		case *ast.FuncDecl:
			sig := FuncSignature{
				Name:     d.Name.Name,
				Exported: ast.IsExported(d.Name.Name),
			}
			if d.Recv != nil && len(d.Recv.List) > 0 {
				sig.Receiver = exprString(d.Recv.List[0].Type)
			}
			if d.Type.Params != nil {
				sig.Params = fieldListString(d.Type.Params)
			}
			if d.Type.Results != nil {
				sig.Returns = fieldListString(d.Type.Results)
			}
			info.Funcs = append(info.Funcs, sig)
			info.Exports = append(info.Exports, SymbolInfo2{
				Name: d.Name.Name, Kind: "func", Exported: sig.Exported,
			})

		case *ast.GenDecl:
			for _, spec := range d.Specs {
				switch s := spec.(type) {
				case *ast.TypeSpec:
					ti := TypeInfo{
						Name: s.Name.Name,
						Kind: typeKindString(s.Type),
					}
					switch t := s.Type.(type) {
					case *ast.StructType:
						for _, field := range t.Fields.List {
							for _, name := range field.Names {
								ti.Fields = append(ti.Fields, name.Name)
							}
						}
					case *ast.InterfaceType:
						for _, method := range t.Methods.List {
							for _, name := range method.Names {
								ti.Methods = append(ti.Methods, name.Name)
							}
						}
					}
					info.Types = append(info.Types, ti)
					info.Exports = append(info.Exports, SymbolInfo2{
						Name: s.Name.Name, Kind: ti.Kind, Exported: ast.IsExported(s.Name.Name),
					})

				case *ast.ValueSpec:
					kind := "var"
					if d.Tok == token.CONST {
						kind = "const"
					}
					for _, name := range s.Names {
						info.Exports = append(info.Exports, SymbolInfo2{
							Name: name.Name, Kind: kind, Exported: ast.IsExported(name.Name),
						})
					}
				}
			}
		}
	}

	return info, nil
}

func exprString(expr ast.Expr) string {
	switch e := expr.(type) {
	case *ast.Ident:
		return e.Name
	case *ast.StarExpr:
		return "*" + exprString(e.X)
	case *ast.SelectorExpr:
		return exprString(e.X) + "." + e.Sel.Name
	default:
		return ""
	}
}

func fieldListString(fl *ast.FieldList) string {
	var parts []string
	for _, f := range fl.List {
		typeStr := exprString(f.Type)
		if len(f.Names) == 0 {
			parts = append(parts, typeStr)
		} else {
			for _, name := range f.Names {
				parts = append(parts, name.Name+" "+typeStr)
			}
		}
	}
	return strings.Join(parts, ", ")
}

func typeKindString(expr ast.Expr) string {
	switch expr.(type) {
	case *ast.StructType:
		return "struct"
	case *ast.InterfaceType:
		return "interface"
	default:
		return "type"
	}
}

// ── TypeScript/JavaScript Parser (regex) ────────────────────────────────────

type TypeScriptParser struct{}

func (p *TypeScriptParser) Language() string { return "typescript" }
func (p *TypeScriptParser) CanParse(path string) bool {
	ext := filepath.Ext(path)
	return ext == ".ts" || ext == ".tsx" || ext == ".js" || ext == ".jsx" || ext == ".mjs" || ext == ".cjs"
}

var (
	tsImportRe     = regexp.MustCompile(`import\s+(?:type\s+)?(?:\{[^}]*\}|\*\s+as\s+\w+|\w+)(?:\s*,\s*(?:\{[^}]*\}|\*\s+as\s+\w+))?\s+from\s+['"]([^'"]+)['"]`)
	tsSideImportRe = regexp.MustCompile(`import\s+['"]([^'"]+)['"]`)
	tsExportRe     = regexp.MustCompile(`export\s+(?:default\s+)?(?:const|let|var|function|class|enum|type|interface|abstract\s+class)\s+(\w+)`)
	tsFuncRe       = regexp.MustCompile(`(?:export\s+)?(?:async\s+)?function\s+(\w+)\s*(?:<[^>]*>)?\s*\(([^)]*)\)(?:\s*:\s*([^{]+))?`)
	tsArrowRe      = regexp.MustCompile(`(?:export\s+)?(?:const|let|var)\s+(\w+)\s*(?::\s*\w+)?\s*=\s*(?:async\s+)?\(([^)]*)\)(?:\s*:\s*([^{=]+))?\s*=>`)
	tsClassRe      = regexp.MustCompile(`(?:export\s+)?(?:abstract\s+)?class\s+(\w+)(?:\s+extends\s+(\w+))?(?:\s+implements\s+([^{]+))?`)
	tsInterfaceRe  = regexp.MustCompile(`(?:export\s+)?interface\s+(\w+)(?:\s+extends\s+([^{]+))?\s*\{([^}]*)\}`)
	tsTypeRe       = regexp.MustCompile(`(?:export\s+)?type\s+(\w+)(?:<[^>]*>)?\s*=\s*([^;]+)`)
	tsEnumRe       = regexp.MustCompile(`(?:export\s+)?enum\s+(\w+)\s*\{([^}]*)\}`)
)

func (p *TypeScriptParser) Parse(path string, content []byte) (*FileInfo, error) {
	src := string(content)
	info := &FileInfo{Path: path, Language: "typescript"}

	seen := make(map[string]bool)
	for _, m := range tsImportRe.FindAllStringSubmatch(src, -1) {
		if !seen[m[1]] {
			info.Imports = append(info.Imports, ImportInfo{Path: m[1]})
			seen[m[1]] = true
		}
	}
	for _, m := range tsSideImportRe.FindAllStringSubmatch(src, -1) {
		if !seen[m[1]] {
			info.Imports = append(info.Imports, ImportInfo{Path: m[1]})
			seen[m[1]] = true
		}
	}

	for _, m := range tsExportRe.FindAllStringSubmatch(src, -1) {
		info.Exports = append(info.Exports, SymbolInfo2{Name: m[1], Kind: "export", Exported: true})
	}

	for _, m := range tsFuncRe.FindAllStringSubmatch(src, -1) {
		exported := strings.Contains(m[0], "export")
		ret := strings.TrimSpace(m[3])
		info.Funcs = append(info.Funcs, FuncSignature{
			Name: m[1], Params: strings.TrimSpace(m[2]), Returns: ret, Exported: exported,
		})
		if !exported {
			info.Exports = append(info.Exports, SymbolInfo2{Name: m[1], Kind: "func", Exported: false})
		}
	}

	for _, m := range tsArrowRe.FindAllStringSubmatch(src, -1) {
		exported := strings.Contains(m[0], "export")
		ret := strings.TrimSpace(m[3])
		info.Funcs = append(info.Funcs, FuncSignature{
			Name: m[1], Params: strings.TrimSpace(m[2]), Returns: ret, Exported: exported,
		})
	}

	for _, m := range tsClassRe.FindAllStringSubmatch(src, -1) {
		exported := strings.Contains(m[0], "export")
		info.Types = append(info.Types, TypeInfo{Name: m[1], Kind: "class"})
		info.Exports = append(info.Exports, SymbolInfo2{Name: m[1], Kind: "class", Exported: exported})
	}

	for _, m := range tsInterfaceRe.FindAllStringSubmatch(src, -1) {
		exported := strings.Contains(m[0], "export")
		var fields []string
		body := m[3]
		for _, line := range strings.Split(body, "\n") {
			line = strings.TrimSpace(line)
			if idx := strings.IndexAny(line, ":?"); idx > 0 {
				fields = append(fields, strings.TrimSpace(line[:idx]))
			}
		}
		info.Types = append(info.Types, TypeInfo{Name: m[1], Kind: "interface", Fields: fields})
		info.Exports = append(info.Exports, SymbolInfo2{Name: m[1], Kind: "interface", Exported: exported})
	}

	for _, m := range tsTypeRe.FindAllStringSubmatch(src, -1) {
		exported := strings.Contains(m[0], "export")
		info.Types = append(info.Types, TypeInfo{Name: m[1], Kind: "type"})
		info.Exports = append(info.Exports, SymbolInfo2{Name: m[1], Kind: "type", Exported: exported})
	}

	for _, m := range tsEnumRe.FindAllStringSubmatch(src, -1) {
		exported := strings.Contains(m[0], "export")
		info.Types = append(info.Types, TypeInfo{Name: m[1], Kind: "enum"})
		info.Exports = append(info.Exports, SymbolInfo2{Name: m[1], Kind: "enum", Exported: exported})
	}

	return info, nil
}

// ── Python Parser (regex) ───────────────────────────────────────────────────

type PythonParser struct{}

func (p *PythonParser) Language() string          { return "python" }
func (p *PythonParser) CanParse(path string) bool { return filepath.Ext(path) == ".py" }

var (
	pyImportRe     = regexp.MustCompile(`^import\s+([\w.]+)`)
	pyFromImportRe = regexp.MustCompile(`^from\s+([\w.]+)\s+import\s+(.+)`)
	pyClassRe      = regexp.MustCompile(`^class\s+(\w+)(?:\(([^)]*)\))?\s*:`)
	pyFuncRe       = regexp.MustCompile(`^(?:async\s+)?def\s+(\w+)\s*\(([^)]*)\)(?:\s*->\s*([^\s:]+))?\s*:`)
	pyDecoratorRe  = regexp.MustCompile(`^@(\w+(?:\.\w+)?)`)
	pyTopLevelRe   = regexp.MustCompile(`^(\w+)\s*=\s*`)
)

func (p *PythonParser) Parse(path string, content []byte) (*FileInfo, error) {
	lines := strings.Split(string(content), "\n")
	info := &FileInfo{Path: path, Language: "python"}

	seen := make(map[string]bool)
	for i, line := range lines {
		trimmed := strings.TrimSpace(line)
		indent := len(line) - len(strings.TrimLeft(line, " \t"))

		if m := pyFromImportRe.FindStringSubmatch(trimmed); m != nil {
			modPath := m[1]
			if !seen[modPath] {
				info.Imports = append(info.Imports, ImportInfo{Path: modPath})
				seen[modPath] = true
			}
			names := strings.Split(m[2], ",")
			for _, name := range names {
				name = strings.TrimSpace(name)
				if name != "" && name != "*" {
					name = strings.SplitN(name, " as ", 2)[0]
					info.Exports = append(info.Exports, SymbolInfo2{
						Name: strings.TrimSpace(name), Kind: "import", Exported: false,
					})
				}
			}
			continue
		}

		if m := pyImportRe.FindStringSubmatch(trimmed); m != nil {
			modPath := m[1]
			if !seen[modPath] {
				info.Imports = append(info.Imports, ImportInfo{Path: modPath})
				seen[modPath] = true
			}
			continue
		}

		if indent > 0 {
			continue
		}

		if m := pyClassRe.FindStringSubmatch(trimmed); m != nil {
			ti := TypeInfo{Name: m[1], Kind: "class"}
			if m[2] != "" {
				bases := strings.Split(m[2], ",")
				for _, b := range bases {
					b = strings.TrimSpace(b)
					if b != "" {
						ti.Fields = append(ti.Fields, b)
					}
				}
			}
			info.Types = append(info.Types, ti)
			info.Exports = append(info.Exports, SymbolInfo2{
				Name: m[1], Kind: "class", Exported: !strings.HasPrefix(m[1], "_"),
			})
			continue
		}

		if m := pyFuncRe.FindStringSubmatch(trimmed); m != nil {
			name := m[1]
			var decorator string
			if i > 0 {
				prevLine := strings.TrimSpace(lines[i-1])
				if dm := pyDecoratorRe.FindStringSubmatch(prevLine); dm != nil {
					decorator = dm[1]
				}
			}
			exported := !strings.HasPrefix(name, "_")
			sig := FuncSignature{
				Name:     name,
				Params:   cleanPyParams(m[2]),
				Returns:  m[3],
				Exported: exported,
			}
			if decorator != "" {
				sig.Receiver = "@" + decorator
			}
			info.Funcs = append(info.Funcs, sig)
			info.Exports = append(info.Exports, SymbolInfo2{
				Name: name, Kind: "func", Exported: exported,
			})
			continue
		}

		if m := pyTopLevelRe.FindStringSubmatch(trimmed); m != nil {
			name := m[1]
			if name != "_" && !strings.HasPrefix(name, "__") {
				info.Exports = append(info.Exports, SymbolInfo2{
					Name: name, Kind: "var", Exported: !strings.HasPrefix(name, "_"),
				})
			}
		}
	}

	return info, nil
}

func cleanPyParams(params string) string {
	params = strings.TrimSpace(params)
	if params == "self" || params == "cls" {
		return ""
	}
	params = strings.TrimPrefix(params, "self,")
	params = strings.TrimPrefix(params, "cls,")
	return strings.TrimSpace(params)
}

// ── Rust Parser (regex) ─────────────────────────────────────────────────────

type RustParser struct{}

func (p *RustParser) Language() string          { return "rust" }
func (p *RustParser) CanParse(path string) bool { return filepath.Ext(path) == ".rs" }

var (
	rustUseRe       = regexp.MustCompile(`(?m)^use\s+([\w:]+(?:\s+as\s+\w+)?)(?:\s*;\s*$|\s*\{)`)
	rustPubFnRe     = regexp.MustCompile(`pub\s+(?:async\s+)?fn\s+(\w+)`)
	rustPubStructRe = regexp.MustCompile(`pub\s+struct\s+(\w+)`)
	rustPubEnumRe   = regexp.MustCompile(`pub\s+enum\s+(\w+)`)
	rustPubTraitRe  = regexp.MustCompile(`pub\s+trait\s+(\w+)`)
	rustPubTypeRe   = regexp.MustCompile(`pub\s+type\s+(\w+)`)
	rustPubConstRe  = regexp.MustCompile(`pub\s+const\s+(\w+)`)
	rustFnRe        = regexp.MustCompile(`(?:pub\s+)?(?:async\s+)?fn\s+(\w+)\s*(?:<[^>]*>)?\s*\(([^)]*)\)(?:\s*->\s*([^{]+))?`)
	rustStructRe    = regexp.MustCompile(`(?:pub\s+)?struct\s+(\w+)(?:<[^>]*>)?\s*(?:\{([^}]*)\}|;)`)
)

func (p *RustParser) Parse(path string, content []byte) (*FileInfo, error) {
	src := string(content)
	info := &FileInfo{Path: path, Language: "rust"}

	seen := make(map[string]bool)
	for _, m := range rustUseRe.FindAllStringSubmatch(src, -1) {
		usePath := m[1]
		if !seen[usePath] {
			info.Imports = append(info.Imports, ImportInfo{Path: usePath})
			seen[usePath] = true
		}
	}

	for _, m := range rustPubFnRe.FindAllStringSubmatch(src, -1) {
		info.Exports = append(info.Exports, SymbolInfo2{Name: m[1], Kind: "func", Exported: true})
	}
	for _, m := range rustPubStructRe.FindAllStringSubmatch(src, -1) {
		info.Exports = append(info.Exports, SymbolInfo2{Name: m[1], Kind: "struct", Exported: true})
	}
	for _, m := range rustPubEnumRe.FindAllStringSubmatch(src, -1) {
		info.Exports = append(info.Exports, SymbolInfo2{Name: m[1], Kind: "enum", Exported: true})
	}
	for _, m := range rustPubTraitRe.FindAllStringSubmatch(src, -1) {
		info.Exports = append(info.Exports, SymbolInfo2{Name: m[1], Kind: "trait", Exported: true})
	}
	for _, m := range rustPubTypeRe.FindAllStringSubmatch(src, -1) {
		info.Exports = append(info.Exports, SymbolInfo2{Name: m[1], Kind: "type", Exported: true})
	}
	for _, m := range rustPubConstRe.FindAllStringSubmatch(src, -1) {
		info.Exports = append(info.Exports, SymbolInfo2{Name: m[1], Kind: "const", Exported: true})
	}

	for _, m := range rustFnRe.FindAllStringSubmatch(src, -1) {
		exported := strings.Contains(m[0], "pub ")
		ret := strings.TrimSpace(m[3])
		info.Funcs = append(info.Funcs, FuncSignature{
			Name: m[1], Params: strings.TrimSpace(m[2]), Returns: ret, Exported: exported,
		})
	}

	for _, m := range rustStructRe.FindAllStringSubmatch(src, -1) {
		ti := TypeInfo{Name: m[1], Kind: "struct"}
		if m[2] != "" {
			for _, line := range strings.Split(m[2], "\n") {
				line = strings.TrimSpace(line)
				if idx := strings.Index(line, ":"); idx > 0 {
					field := strings.TrimSpace(line[:idx])
					if field != "" && !strings.HasPrefix(field, "//") {
						ti.Fields = append(ti.Fields, strings.TrimPrefix(field, "pub "))
					}
				}
			}
		}
		info.Types = append(info.Types, ti)
	}

	return info, nil
}

// IsExported reports whether a symbol name is exported in its language.
func IsExported(name string, language string) bool {
	if name == "" {
		return false
	}
	switch language {
	case "go":
		return unicode.IsUpper(rune(name[0]))
	case "python":
		return !strings.HasPrefix(name, "_")
	default:
		return !strings.HasPrefix(name, "_")
	}
}
