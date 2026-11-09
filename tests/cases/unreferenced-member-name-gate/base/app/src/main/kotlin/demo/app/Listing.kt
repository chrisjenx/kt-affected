package demo.app

import demo.core.shelf

// `it` has an inferred type: the call gets no edge, only the name.
fun listing(): List<String> = shelf().map { it.display() }
