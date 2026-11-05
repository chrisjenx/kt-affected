package demo

class Formatting {
    fun area(w: Int, h: Int): Int =
        w   *   h
}

class Commented {
    // Doubles its input.
    fun twice(n: Int): Int = n * 2 // no overflow check
}

class Documented {
    /** Rounds toward zero. */
    fun half(n: Int): Int = n / 2
}

class LocalRename {
    fun sum(xs: List<Int>): Int {
        val result = xs.sum()
        return result
    }
}

class TrailingComma {
    fun pair(
        a: Int,
        b: Int,
    ): Int = a + b
}

class Suppressed {
    @Suppress("MagicNumber")
    fun magic(): Int = 42
}
