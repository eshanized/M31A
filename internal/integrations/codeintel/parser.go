package codeintel

import (
	"path/filepath"
	"regexp"
	"strings"
	"unicode"

	gotreesitter "github.com/odvcencio/gotreesitter"
	"github.com/odvcencio/gotreesitter/grammars"
)

// ImportInfo represents a single import declaration in a source file.
type ImportInfo struct {
	Path       string // raw import path (e.g. "fmt", "./utils", "github.com/foo/bar")
	ResolvedTo string // resolved local file path (populated during graph build)
}

// SymbolInfo represents a named symbol exported or defined in a file.
type SymbolInfo struct {
	Name     string
	Kind     string // "func", "type", "interface", "const", "var", "class", "enum", "trait"
	Exported bool
}

// FuncSignature represents a function or method signature.
type FuncSignature struct {
	Name     string
	Receiver string // non-empty for methods (e.g. "*Engine")
	Params   string // parameter list as display string
	Returns  string // return type(s) as display string
	Exported bool
}

// TypeInfo represents a struct, interface, class, or enum definition.
type TypeInfo struct {
	Name    string
	Kind    string // "struct", "interface", "type", "class", "enum", "trait"
	Fields  []string
	Methods []string
}

// FileInfo is the parsed representation of a single source file.
type FileInfo struct {
	Path      string
	Language  string
	Imports   []ImportInfo
	Exports   []SymbolInfo
	Funcs     []FuncSignature
	Types     []TypeInfo
	CallSites []CallSiteInfo // extracted call expressions for call graph
}

// Parser extracts structured information from a source file.
type Parser interface {
	Language() string
	CanParse(path string) bool
	Parse(path string, content []byte) (*FileInfo, error)
}

// AllParsers returns the default set of language parsers.
// Tree-sitter is last so regex parsers take priority for Python/Rust.
func AllParsers() []Parser {
	return []Parser{
		&PythonParser{},
		&RustParser{},
		&TreeSitterParser{},
	}
}

// ParserForFile returns the first parser that can handle the given file, or nil.
func ParserForFile(path string, parsers []Parser) Parser {
	for _, p := range parsers {
		if p.CanParse(path) {
			return p
		}
	}
	return nil
}

// ── Tree-Sitter Parser (gotreesitter, pure Go) ──────────────────────────────

// supportedTreeSitterLangs limits tree-sitter to languages where we extract meaningful info.
var supportedTreeSitterLangs = map[string]bool{
	"go": true, "typescript": true, "javascript": true,
	"python": true, "rust": true, "java": true,
	"cpp": true, "c": true, "c_sharp": true,
	"ruby": true, "php": true, "swift": true, "kotlin": true,
}

// TreeSitterParser uses gotreesitter for multi-language parsing.
type TreeSitterParser struct{}

func (p *TreeSitterParser) Language() string { return "" }

func (p *TreeSitterParser) CanParse(path string) bool {
	entry := grammars.DetectLanguage(filepath.Base(path))
	if entry == nil {
		return false
	}
	return supportedTreeSitterLangs[entry.Name]
}

func (p *TreeSitterParser) Parse(path string, content []byte) (*FileInfo, error) {
	entry := grammars.DetectLanguage(filepath.Base(path))
	if entry == nil {
		return nil, nil
	}
	if !supportedTreeSitterLangs[entry.Name] {
		return nil, nil
	}

	lang := entry.Language()
	parser := gotreesitter.NewParser(lang)
	tree, err := parser.Parse(content)
	if err != nil {
		return nil, err
	}
	defer tree.Release()

	info := &FileInfo{
		Path:     path,
		Language: entry.Name,
	}

	root := tree.RootNode()
	walkAST(root, content, lang, entry.Name, info)

	return info, nil
}

// walkAST traverses the syntax tree and extracts imports, functions, types, and exports.
func walkAST(node *gotreesitter.Node, content []byte, lang *gotreesitter.Language, langName string, info *FileInfo) {
	if node == nil {
		return
	}

	nodeType := node.Type(lang)

	switch nodeType {
	case "import_declaration", "import_statement":
		extractImport(node, content, lang, langName, info)
	case "use_declaration":
		extractUseDeclaration(node, content, lang, langName, info)
	case "export_statement":
		// TS/JS: export wraps declaration — recurse into children
		for i := 0; i < int(node.ChildCount()); i++ {
			child := node.Child(i)
			if child != nil && child.IsNamed() && child.Type(lang) != "export" {
				walkAST(child, content, lang, langName, info)
			}
		}
		return
	case "function_declaration", "method_declaration", "function":
		extractFunction(node, content, lang, langName, info)
	case "type_declaration", "type_spec":
		extractType(node, content, lang, langName, info)
	case "struct_declaration":
		extractStruct(node, content, lang, langName, info)
	case "interface_declaration":
		extractInterface(node, content, lang, langName, info)
	case "class_declaration":
		extractClass(node, content, lang, langName, info)
	case "enum_declaration":
		extractEnum(node, content, lang, langName, info)
	case "const_declaration", "var_declaration", "lexical_declaration":
		extractConstVar(node, content, lang, langName, info)
	case "function_definition", "async_function_definition":
		extractPythonFunc(node, content, lang, info)
	case "class_definition":
		extractPythonClass(node, content, lang, langName, info)
	case "decorated_definition":
		for i := 0; i < int(node.NamedChildCount()); i++ {
			walkAST(node.NamedChild(i), content, lang, langName, info)
		}
		return
	case "call_expression", "function_call", "method_invocation":
		extractCallSite(node, content, lang, langName, info)
	}

	for i := 0; i < int(node.ChildCount()); i++ {
		child := node.Child(i)
		if child != nil && child.IsNamed() {
			walkAST(child, content, lang, langName, info)
		}
	}
}

// extractCallSite extracts call site information from call expressions.
func extractCallSite(node *gotreesitter.Node, content []byte, lang *gotreesitter.Language, langName string, info *FileInfo) {
	// Find the enclosing function to get the caller name
	callerName := findEnclosingFunction(node, content, lang)

	// Extract the callee name from the call expression
	calleeName := extractCalleeName(node, content, lang)

	if callerName != "" && calleeName != "" {
		// Get the line number
		line := int(node.StartPoint().Row) + 1 // 1-indexed

		info.CallSites = append(info.CallSites, CallSiteInfo{
			CallerName: callerName,
			CalleeName: calleeName,
			Line:       line,
		})
	}
}

// findEnclosingFunction walks up the AST to find the enclosing function declaration.
func findEnclosingFunction(node *gotreesitter.Node, content []byte, lang *gotreesitter.Language) string {
	current := node.Parent()
	for current != nil {
		nodeType := current.Type(lang)
		if nodeType == "function_declaration" || nodeType == "method_declaration" ||
			nodeType == "function" || nodeType == "function_definition" ||
			nodeType == "async_function_definition" {
			if nameNode := current.ChildByFieldName("name", lang); nameNode != nil {
				return nameNode.Text(content)
			}
			return childByName(current, content, lang, "identifier", "property_identifier", "name")
		}
		current = current.Parent()
	}
	return ""
}

// extractCalleeName extracts the callee name from a call expression node.
func extractCalleeName(node *gotreesitter.Node, content []byte, lang *gotreesitter.Language) string {
	// Try to get the function being called - could be a field "function" or "method" or direct child
	if fn := node.ChildByFieldName("function", lang); fn != nil {
		return extractFunctionName(fn, content, lang)
	}
	if method := node.ChildByFieldName("method", lang); method != nil {
		return method.Text(content)
	}
	// For some languages, the first named child is the function
	for i := 0; i < int(node.NamedChildCount()); i++ {
		child := node.NamedChild(i)
		if child != nil {
			return extractFunctionName(child, content, lang)
		}
	}
	return ""
}

// extractFunctionName extracts a function name from various node types.
func extractFunctionName(node *gotreesitter.Node, content []byte, lang *gotreesitter.Language) string {
	nodeType := node.Type(lang)
	switch nodeType {
	case "identifier", "property_identifier", "field_identifier", "member_expression":
		return node.Text(content)
	case "call_expression", "function_call", "method_invocation":
		// Nested call - get the function of the nested call
		if fn := node.ChildByFieldName("function", lang); fn != nil {
			return extractFunctionName(fn, content, lang)
		}
	}
	// Try to get name field
	if nameNode := node.ChildByFieldName("name", lang); nameNode != nil {
		return nameNode.Text(content)
	}
	return childByName(node, content, lang, "identifier", "property_identifier", "name")
}

// extractImport handles Go/TS/JS import declarations.
func extractImport(node *gotreesitter.Node, content []byte, lang *gotreesitter.Language, langName string, info *FileInfo) {
	seen := make(map[string]bool)

	// For Go: import_declaration contains import_spec_list > import_spec > import_path
	// For TS/JS: import_statement has source field
	for i := 0; i < int(node.NamedChildCount()); i++ {
		child := node.NamedChild(i)
		if child == nil {
			continue
		}
		childType := child.Type(lang)

		switch childType {
		case "import_spec", "import_spec_list":
			for j := 0; j < int(child.NamedChildCount()); j++ {
				spec := child.NamedChild(j)
				if spec == nil {
					continue
				}
				specType := spec.Type(lang)
				switch specType {
				case "import_path", "interpreted_string_literal", "raw_string_literal":
					path := extractStringContent(spec, content, lang)
					if path != "" && !seen[path] {
						info.Imports = append(info.Imports, ImportInfo{Path: path})
						seen[path] = true
					}
				case "import_spec":
					if importPath := spec.ChildByFieldName("path", lang); importPath != nil {
						path := extractStringContent(importPath, content, lang)
						if path != "" && !seen[path] {
							info.Imports = append(info.Imports, ImportInfo{Path: path})
							seen[path] = true
						}
					}
				}
			}
		case "import_path", "source", "module":
			path := extractStringContent(child, content, lang)
			if path != "" && !seen[path] {
				info.Imports = append(info.Imports, ImportInfo{Path: path})
				seen[path] = true
			}
		case "interpreted_string_literal", "raw_string_literal", "string":
			path := extractStringContent(child, content, lang)
			if path != "" && !seen[path] {
				info.Imports = append(info.Imports, ImportInfo{Path: path})
				seen[path] = true
			}
		}
	}
}

// extractUseDeclaration handles Rust use statements.
func extractUseDeclaration(node *gotreesitter.Node, content []byte, lang *gotreesitter.Language, langName string, info *FileInfo) {
	text := node.Text(content)
	// Extract "use crate::..." path
	if idx := strings.Index(text, "use "); idx >= 0 {
		rest := text[idx+4:]
		rest = strings.TrimSpace(rest)
		// Remove trailing semicolon and braces
		rest = strings.TrimRight(rest, ";{")
		rest = strings.TrimSpace(rest)
		if strings.HasPrefix(rest, "crate::") {
			path := strings.ReplaceAll(rest, "::", "/")
			path = strings.TrimPrefix(path, "crate/")
			info.Imports = append(info.Imports, ImportInfo{Path: path})
		}
	}
}

// extractFunction handles Go/TS/JS function declarations.
func extractFunction(node *gotreesitter.Node, content []byte, lang *gotreesitter.Language, langName string, info *FileInfo) {
	name := ""
	if nameNode := node.ChildByFieldName("name", lang); nameNode != nil {
		name = nameNode.Text(content)
	}
	if name == "" {
		name = childByName(node, content, lang, "identifier", "property_identifier")
	}
	if name == "" {
		return
	}

	exported := isExported(name, langName)
	sig := FuncSignature{Name: name, Exported: exported}

	// Extract parameters
	if params := node.ChildByFieldName("parameters", lang); params != nil {
		sig.Params = strings.Trim(params.Text(content), "()")
	}

	// Extract return type
	if result := node.ChildByFieldName("result", lang); result != nil {
		sig.Returns = strings.Trim(result.Text(content), "()")
	}

	// Extract receiver (Go methods)
	if receiver := node.ChildByFieldName("receiver", lang); receiver != nil {
		sig.Receiver = strings.Trim(receiver.Text(content), "()")
	}

	info.Funcs = append(info.Funcs, sig)
	info.Exports = append(info.Exports, SymbolInfo{
		Name:     name,
		Kind:     "func",
		Exported: exported,
	})
}

// extractType handles Go type declarations.
func extractType(node *gotreesitter.Node, content []byte, lang *gotreesitter.Language, langName string, info *FileInfo) {
	for i := 0; i < int(node.NamedChildCount()); i++ {
		child := node.NamedChild(i)
		if child == nil {
			continue
		}
		childType := child.Type(lang)
		if childType == "type_spec" || childType == "type_identifier" {
			name := ""
			if nameNode := child.ChildByFieldName("name", lang); nameNode != nil {
				name = nameNode.Text(content)
			}
			if name == "" {
				name = child.Text(content)
			}
			if name != "" {
				kind := "type"
				typeNode := child.ChildByFieldName("type", lang)
				if typeNode != nil {
					kind = typeNode.Type(lang)
				}
				info.Types = append(info.Types, TypeInfo{Name: name, Kind: kind})
				info.Exports = append(info.Exports, SymbolInfo{
					Name:     name,
					Kind:     kind,
					Exported: isExported(name, langName),
				})
			}
		}
	}
}

// extractStruct handles struct declarations (TS/JS/Rust).
func extractStruct(node *gotreesitter.Node, content []byte, lang *gotreesitter.Language, langName string, info *FileInfo) {
	name := ""
	if nameNode := node.ChildByFieldName("name", lang); nameNode != nil {
		name = nameNode.Text(content)
	}
	if name == "" {
		name = childByName(node, content, lang, "type_identifier", "identifier")
	}
	if name == "" {
		return
	}

	ti := TypeInfo{Name: name, Kind: "struct"}

	// Extract fields
	body := node.ChildByFieldName("body", lang)
	if body == nil {
		for i := 0; i < int(node.NamedChildCount()); i++ {
			child := node.NamedChild(i)
			if child != nil && (child.Type(lang) == "field_declaration_list" || child.Type(lang) == "class_body" || child.Type(lang) == "block") {
				body = child
				break
			}
		}
	}
	if body != nil {
		for i := 0; i < int(body.NamedChildCount()); i++ {
			field := body.NamedChild(i)
			if field != nil {
				fieldType := field.Type(lang)
				if fieldType == "field_declaration" || fieldType == "property_definition" || fieldType == "field_definition" || fieldType == "property_signature" {
					if fname := fieldByName(field, content, lang); fname != "" {
						ti.Fields = append(ti.Fields, fname)
					}
				}
			}
		}
	}

	info.Types = append(info.Types, ti)
	info.Exports = append(info.Exports, SymbolInfo{
		Name:     name,
		Kind:     "struct",
		Exported: isExported(name, langName),
	})
}

// extractInterface handles interface declarations.
func extractInterface(node *gotreesitter.Node, content []byte, lang *gotreesitter.Language, langName string, info *FileInfo) {
	name := ""
	if nameNode := node.ChildByFieldName("name", lang); nameNode != nil {
		name = nameNode.Text(content)
	}
	if name == "" {
		name = childByName(node, content, lang, "type_identifier", "identifier")
	}
	if name == "" {
		return
	}

	ti := TypeInfo{Name: name, Kind: "interface"}

	// Extract methods and properties
	body := node.ChildByFieldName("body", lang)
	if body != nil {
		for i := 0; i < int(body.NamedChildCount()); i++ {
			member := body.NamedChild(i)
			if member == nil {
				continue
			}
			memberType := member.Type(lang)
			switch memberType {
			case "method_signature":
				if methodName := fieldByName(member, content, lang); methodName != "" {
					ti.Methods = append(ti.Methods, methodName)
				}
			case "property_signature":
				if fieldName := fieldByName(member, content, lang); fieldName != "" {
					ti.Fields = append(ti.Fields, fieldName)
				}
			}
		}
	}

	info.Types = append(info.Types, ti)
	info.Exports = append(info.Exports, SymbolInfo{
		Name:     name,
		Kind:     "interface",
		Exported: isExported(name, langName),
	})
}

// extractClass handles class declarations.
func extractClass(node *gotreesitter.Node, content []byte, lang *gotreesitter.Language, langName string, info *FileInfo) {
	name := ""
	// Try "name" field first (Go), then look for type_identifier/identifier child (TS)
	if nameNode := node.ChildByFieldName("name", lang); nameNode != nil {
		name = nameNode.Text(content)
	}
	if name == "" {
		name = childByName(node, content, lang, "type_identifier", "identifier", "name")
	}
	if name == "" {
		return
	}

	ti := TypeInfo{Name: name, Kind: "class"}

	// Extract fields/properties
	body := node.ChildByFieldName("body", lang)
	if body != nil {
		for i := 0; i < int(body.NamedChildCount()); i++ {
			member := body.NamedChild(i)
			if member == nil {
				continue
			}
			memberType := member.Type(lang)
			switch memberType {
			case "field_definition", "property_definition", "property_signature":
				if fieldName := fieldByName(member, content, lang); fieldName != "" {
					ti.Fields = append(ti.Fields, fieldName)
				}
			case "method_definition", "method_signature":
				if methodName := fieldByName(member, content, lang); methodName != "" {
					ti.Methods = append(ti.Methods, methodName)
				}
			}
		}
	}

	info.Types = append(info.Types, ti)
	info.Exports = append(info.Exports, SymbolInfo{
		Name:     name,
		Kind:     "class",
		Exported: isExported(name, langName),
	})
}

// extractEnum handles enum declarations.
func extractEnum(node *gotreesitter.Node, content []byte, lang *gotreesitter.Language, langName string, info *FileInfo) {
	name := ""
	if nameNode := node.ChildByFieldName("name", lang); nameNode != nil {
		name = nameNode.Text(content)
	}
	if name == "" {
		name = childByName(node, content, lang, "identifier", "type_identifier")
	}
	if name == "" {
		return
	}

	info.Types = append(info.Types, TypeInfo{Name: name, Kind: "enum"})
	info.Exports = append(info.Exports, SymbolInfo{
		Name:     name,
		Kind:     "enum",
		Exported: isExported(name, langName),
	})
}

// extractConstVar handles const/var declarations (Go, TS).
func extractConstVar(node *gotreesitter.Node, content []byte, lang *gotreesitter.Language, langName string, info *FileInfo) {
	nodeType := node.Type(lang)
	kind := "var"
	if nodeType == "const_declaration" || (nodeType == "lexical_declaration" && strings.HasPrefix(strings.TrimSpace(node.Text(content)), "const")) {
		kind = "const"
	}

	for i := 0; i < int(node.NamedChildCount()); i++ {
		child := node.NamedChild(i)
		if child == nil {
			continue
		}
		childType := child.Type(lang)
		if childType == "const_spec" || childType == "var_spec" || childType == "variable_declarator" {
			name := ""
			if nameNode := child.ChildByFieldName("name", lang); nameNode != nil {
				name = nameNode.Text(content)
			}
			if name == "" {
				name = childByName(child, content, lang, "identifier", "type_identifier")
			}
			if name != "" {
				info.Exports = append(info.Exports, SymbolInfo{
					Name:     name,
					Kind:     kind,
					Exported: isExported(name, langName),
				})
			}
		}
	}
}

// extractPythonFunc handles Python function definitions.
func extractPythonFunc(node *gotreesitter.Node, content []byte, lang *gotreesitter.Language, info *FileInfo) {
	name := ""
	if nameNode := node.ChildByFieldName("name", lang); nameNode != nil {
		name = nameNode.Text(content)
	}
	if name == "" {
		return
	}

	exported := !strings.HasPrefix(name, "_")
	sig := FuncSignature{Name: name, Exported: exported}

	if params := node.ChildByFieldName("parameters", lang); params != nil {
		sig.Params = strings.Trim(params.Text(content), "()")
	}

	if returnType := node.ChildByFieldName("return_type", lang); returnType != nil {
		sig.Returns = strings.TrimPrefix(returnType.Text(content), "->")
		sig.Returns = strings.TrimSpace(sig.Returns)
	}

	info.Funcs = append(info.Funcs, sig)
	info.Exports = append(info.Exports, SymbolInfo{
		Name:     name,
		Kind:     "func",
		Exported: exported,
	})
}

// extractPythonClass handles Python class definitions.
func extractPythonClass(node *gotreesitter.Node, content []byte, lang *gotreesitter.Language, langName string, info *FileInfo) {
	name := ""
	if nameNode := node.ChildByFieldName("name", lang); nameNode != nil {
		name = nameNode.Text(content)
	}
	if name == "" {
		return
	}

	ti := TypeInfo{Name: name, Kind: "class"}

	// Extract superclass
	if superclasses := node.ChildByFieldName("superclasses", lang); superclasses != nil {
		for i := 0; i < int(superclasses.NamedChildCount()); i++ {
			arg := superclasses.NamedChild(i)
			if arg != nil {
				ti.Fields = append(ti.Fields, arg.Text(content))
			}
		}
	}

	info.Types = append(info.Types, ti)
	info.Exports = append(info.Exports, SymbolInfo{
		Name:     name,
		Kind:     "class",
		Exported: !strings.HasPrefix(name, "_"),
	})
}

// childByName finds the first named child matching any of the given types.
func childByName(node *gotreesitter.Node, content []byte, lang *gotreesitter.Language, types ...string) string {
	for i := 0; i < int(node.NamedChildCount()); i++ {
		child := node.NamedChild(i)
		if child == nil {
			continue
		}
		childType := child.Type(lang)
		for _, t := range types {
			if childType == t {
				return child.Text(content)
			}
		}
	}
	return ""
}

// fieldByName tries ChildByFieldName("name"), then falls back to identifier-like children.
func fieldByName(node *gotreesitter.Node, content []byte, lang *gotreesitter.Language) string {
	if nameNode := node.ChildByFieldName("name", lang); nameNode != nil {
		return nameNode.Text(content)
	}
	return childByName(node, content, lang, "property_identifier", "identifier", "type_identifier")
}

// extractStringContent extracts the string content from a string literal node.
func extractStringContent(node *gotreesitter.Node, content []byte, lang *gotreesitter.Language) string {
	text := node.Text(content)
	// Remove quotes
	if len(text) >= 2 {
		if (text[0] == '"' && text[len(text)-1] == '"') ||
			(text[0] == '\'' && text[len(text)-1] == '\'') ||
			(text[0] == '`' && text[len(text)-1] == '`') {
			text = text[1 : len(text)-1]
		}
	}
	return text
}

// isExported determines if a symbol name is exported based on language rules.
func isExported(name string, language string) bool {
	if name == "" {
		return false
	}
	switch language {
	case "go":
		return unicode.IsUpper(rune(name[0]))
	case "python":
		return !strings.HasPrefix(name, "_")
	case "rust":
		return !strings.HasPrefix(name, "_")
	default:
		// TypeScript/JavaScript: assume exported if not prefixed with _
		return !strings.HasPrefix(name, "_")
	}
}

// ── Python Parser (regex, for files not handled well by tree-sitter) ────────

// PythonParser extracts imports, classes, and functions from Python files.
type PythonParser struct{}

func (p *PythonParser) Language() string { return "python" }

func (p *PythonParser) CanParse(path string) bool {
	return filepath.Ext(path) == ".py"
}

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
	info := &FileInfo{
		Path:     path,
		Language: "python",
	}

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
					info.Exports = append(info.Exports, SymbolInfo{
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
			info.Exports = append(info.Exports, SymbolInfo{
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
			info.Exports = append(info.Exports, SymbolInfo{
				Name: name, Kind: "func", Exported: exported,
			})
			continue
		}

		if m := pyTopLevelRe.FindStringSubmatch(trimmed); m != nil {
			name := m[1]
			if name != "_" && !strings.HasPrefix(name, "__") {
				info.Exports = append(info.Exports, SymbolInfo{
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

// ── Rust Parser (regex, for files not handled well by tree-sitter) ──────────

// RustParser extracts use statements, pub items, and type definitions from Rust files.
type RustParser struct{}

func (p *RustParser) Language() string { return "rust" }

func (p *RustParser) CanParse(path string) bool {
	return filepath.Ext(path) == ".rs"
}

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
	info := &FileInfo{
		Path:     path,
		Language: "rust",
	}

	seen := make(map[string]bool)
	for _, m := range rustUseRe.FindAllStringSubmatch(src, -1) {
		usePath := m[1]
		if !seen[usePath] {
			info.Imports = append(info.Imports, ImportInfo{Path: usePath})
			seen[usePath] = true
		}
	}

	for _, m := range rustPubFnRe.FindAllStringSubmatch(src, -1) {
		info.Exports = append(info.Exports, SymbolInfo{Name: m[1], Kind: "func", Exported: true})
	}
	for _, m := range rustPubStructRe.FindAllStringSubmatch(src, -1) {
		info.Exports = append(info.Exports, SymbolInfo{Name: m[1], Kind: "struct", Exported: true})
	}
	for _, m := range rustPubEnumRe.FindAllStringSubmatch(src, -1) {
		info.Exports = append(info.Exports, SymbolInfo{Name: m[1], Kind: "enum", Exported: true})
	}
	for _, m := range rustPubTraitRe.FindAllStringSubmatch(src, -1) {
		info.Exports = append(info.Exports, SymbolInfo{Name: m[1], Kind: "trait", Exported: true})
	}
	for _, m := range rustPubTypeRe.FindAllStringSubmatch(src, -1) {
		info.Exports = append(info.Exports, SymbolInfo{Name: m[1], Kind: "type", Exported: true})
	}
	for _, m := range rustPubConstRe.FindAllStringSubmatch(src, -1) {
		info.Exports = append(info.Exports, SymbolInfo{Name: m[1], Kind: "const", Exported: true})
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

	for _, m := range rustPubEnumRe.FindAllStringSubmatch(src, -1) {
		info.Types = append(info.Types, TypeInfo{Name: m[1], Kind: "enum"})
	}
	for _, m := range rustPubTraitRe.FindAllStringSubmatch(src, -1) {
		info.Types = append(info.Types, TypeInfo{Name: m[1], Kind: "trait"})
	}

	return info, nil
}
