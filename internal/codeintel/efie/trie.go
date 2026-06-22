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
