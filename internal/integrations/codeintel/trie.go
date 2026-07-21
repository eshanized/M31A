package codeintel

import "strings"

// TrieNode represents a node in the Trie data structure.
type TrieNode struct {
	children [128]*TrieNode // ASCII-only for symbol names
	symbols  []string       // symbols sharing this prefix (at leaf)
	isEnd    bool
}

// SymbolTrie provides O(K) prefix-based symbol search replacing O(N) linear scan.
type SymbolTrie struct {
	root *TrieNode
}

// NewSymbolTrie creates a new empty Trie.
func NewSymbolTrie() *SymbolTrie {
	return &SymbolTrie{root: &TrieNode{}}
}

// Insert adds a symbol name to the Trie. O(K) where K = name length.
func (t *SymbolTrie) Insert(name string) {
	current := t.root
	lower := strings.ToLower(name)
	for i := 0; i < len(lower); i++ {
		c := lower[i]
		if c >= 128 {
			continue // skip non-ASCII
		}
		if current.children[c] == nil {
			current.children[c] = &TrieNode{}
		}
		current = current.children[c]
	}
	current.isEnd = true
	current.symbols = append(current.symbols, name)
}

// Search performs exact match lookup. O(K) time.
func (t *SymbolTrie) Search(name string) bool {
	current := t.root
	lower := strings.ToLower(name)
	for i := 0; i < len(lower); i++ {
		c := lower[i]
		if c >= 128 {
			return false
		}
		if current.children[c] == nil {
			return false
		}
		current = current.children[c]
	}
	return current.isEnd
}

// HasPrefix checks if any symbol has the given prefix. O(K) time.
func (t *SymbolTrie) HasPrefix(prefix string) bool {
	current := t.root
	lower := strings.ToLower(prefix)
	for i := 0; i < len(lower); i++ {
		c := lower[i]
		if c >= 128 {
			return false
		}
		if current.children[c] == nil {
			return false
		}
		current = current.children[c]
	}
	return true
}

// PrefixSearch returns all symbols with the given prefix. O(K + M) time where M = matches.
func (t *SymbolTrie) PrefixSearch(prefix string) []string {
	current := t.root
	lower := strings.ToLower(prefix)
	for i := 0; i < len(lower); i++ {
		c := lower[i]
		if c >= 128 {
			return nil
		}
		if current.children[c] == nil {
			return nil
		}
		current = current.children[c]
	}
	var result []string
	t.collectSymbols(current, &result)
	return result
}

// collectSymbols gathers all symbols under a node.
func (t *SymbolTrie) collectSymbols(node *TrieNode, result *[]string) {
	if node == nil {
		return
	}
	if node.isEnd {
		*result = append(*result, node.symbols...)
	}
	for i := 0; i < 128; i++ {
		if node.children[i] != nil {
			t.collectSymbols(node.children[i], result)
		}
	}
}

// Size returns the total number of symbols in the Trie.
func (t *SymbolTrie) Size() int {
	return t.countNodes(t.root)
}

// Delete removes a symbol name from the Trie.
func (t *SymbolTrie) Delete(name string) {
	current := t.root
	lower := strings.ToLower(name)
	for i := 0; i < len(lower); i++ {
		c := lower[i]
		if c >= 128 {
			return
		}
		if current.children[c] == nil {
			return
		}
		current = current.children[c]
	}
	if !current.isEnd {
		return
	}
	for i, s := range current.symbols {
		if s == name {
			current.symbols = append(current.symbols[:i], current.symbols[i+1:]...)
			break
		}
	}
	if len(current.symbols) == 0 {
		current.isEnd = false
	}
}

func (t *SymbolTrie) countNodes(node *TrieNode) int {
	if node == nil {
		return 0
	}
	count := 0
	if node.isEnd {
		count += len(node.symbols)
	}
	for i := 0; i < 128; i++ {
		count += t.countNodes(node.children[i])
	}
	return count
}
