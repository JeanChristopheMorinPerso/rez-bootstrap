package main

import "fmt"

type Config int

// Note that the order is significant.
// We use the order to determine which interpreter is the most optmized.
const (
	PgoLto Config = iota
	PGO
	LTO
	NoOpt
	Empty
	Debug
)

type ByConfig []Interpreter

func (bc ByConfig) Len() int           { return len(bc) }
func (bc ByConfig) Swap(a, b int)      { bc[a], bc[b] = bc[b], bc[a] }
func (bc ByConfig) Less(a, b int) bool { return bc[a].Config < bc[b].Config }

func ConfigFromString(s string) (Config, error) {
	switch s {
	case "debug":
		return Debug, nil
	case "pgo+lto":
		return PgoLto, nil
	case "lto":
		return LTO, nil
	case "noopt":
		return NoOpt, nil
	case "pgo":
		return PGO, nil
	case "":
		return Empty, nil
	default:
		return -1, fmt.Errorf("unknown config: %q", s)
	}
}

func (c Config) ToString() string {
	var str string

	switch c {
	case Debug:
		str = "debug"
	case PgoLto:
		str = "pgo+lto"
	case LTO:
		str = "lto"
	case NoOpt:
		str = "noopt"
	case PGO:
		str = "pgo"
	case Empty:
		str = ""
	}
	return str
}
