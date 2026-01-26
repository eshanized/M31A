// Package bisect wraps git bisect to automatically identify the commit that
// introduced a regression. It runs a user-supplied check function across a
// commit range and returns the offending commit with its diff.
package bisect
