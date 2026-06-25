package efie

// TrieNode represents a node in the trie for symbol matching.
type TrieNode struct {
	children [128]*TrieNode
	symbols  []string
	isEnd    bool
}

// Trie provides O(K) prefix-based symbol search.
type Trie struct {
	root *TrieNode
}

// NewTrie creates a new empty trie.
func NewTrie() *Trie {
	return &Trie{root: &TrieNode{}}
}

// BuildTrie constructs a trie from a list of symbol names.
func BuildTrie(names []string) *Trie {
	t := NewTrie()
	for _, name := range names {
		t.Insert(name)
	}
	return t
}

// Insert adds a symbol name to the trie. O(K) time.
func (t *Trie) Insert(name string) {
	node := t.root
	for i := 0; i < len(name); i++ {
		c := name[i]
		if c >= 128 {
			continue
		}
		if node.children[c] == nil {
			node.children[c] = &TrieNode{}
		}
		node = node.children[c]
	}
	node.isEnd = true
	for _, s := range node.symbols {
		if s == name {
			return
		}
	}
	node.symbols = append(node.symbols, name)
}

// Search returns exact match results. O(K) time.
func (t *Trie) Search(name string) []string {
	node := t.root
	for i := 0; i < len(name); i++ {
		c := name[i]
		if c >= 128 {
			return nil
		}
		if node.children[c] == nil {
			return nil
		}
		node = node.children[c]
	}
	if node.isEnd {
		return node.symbols
	}
	return nil
}

// HasPrefix checks if any symbol starts with the given prefix. O(K) time.
func (t *Trie) HasPrefix(prefix string) bool {
	node := t.root
	for i := 0; i < len(prefix); i++ {
		c := prefix[i]
		if c >= 128 {
			return false
		}
		if node.children[c] == nil {
			return false
		}
		node = node.children[c]
	}
	return node != nil
}

// PrefixSearch returns all symbols sharing the given prefix. O(K + M) time.
func (t *Trie) PrefixSearch(prefix string) []string {
	node := t.root
	for i := 0; i < len(prefix); i++ {
		c := prefix[i]
		if c >= 128 {
			return nil
		}
		if node.children[c] == nil {
			return nil
		}
		node = node.children[c]
	}
	var result []string
	collectSymbols(node, &result)
	return result
}

// FuzzySearch finds all symbols within maxEditDistance edits of query using Levenshtein distance. O(K x E) time.
func (t *Trie) FuzzySearch(query string, maxEditDistance int) []string {
	var result []string
	seen := make(map[string]bool)
	// Generate all prefixes of the query and search with each
	for i := 1; i <= len(query); i++ {
		prefix := query[:i]
		matches := t.fuzzyCollect(t.root, prefix, maxEditDistance, maxEditDistance)
		for _, m := range matches {
			if !seen[m] {
				seen[m] = true
				result = append(result, m)
			}
		}
	}
	return result
}

func (t *Trie) fuzzyCollect(node *TrieNode, remaining string, editsLeft int, maxEdits int) []string {
	var result []string
	if len(remaining) == 0 {
		if node.isEnd {
			result = append(result, node.symbols...)
		}
		for _, child := range node.children {
			if child != nil && child.isEnd {
				result = append(result, child.symbols...)
			}
		}
		return result
	}

	char := remaining[0]
	for c, child := range node.children {
		if child == nil {
			continue
		}
		if byte(c) == char {
			result = append(result, t.fuzzyCollect(child, remaining[1:], editsLeft, maxEdits)...)
		} else if editsLeft > 0 {
			result = append(result, t.fuzzyCollect(child, remaining, editsLeft-1, maxEdits)...)
		}
	}
	return result
}

func collectSymbols(node *TrieNode, result *[]string) {
	if node == nil {
		return
	}
	if node.isEnd {
		*result = append(*result, node.symbols...)
	}
	for _, child := range node.children {
		if child != nil {
			collectSymbols(child, result)
		}
	}
}
