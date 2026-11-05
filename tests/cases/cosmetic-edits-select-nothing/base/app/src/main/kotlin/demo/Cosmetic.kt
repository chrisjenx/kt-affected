package demo

class Formatting {
    fun area(w: Int, h: Int): Int = w * h
}

class Commented {
    fun twice(n: Int): Int = n * 2
}

class Documented {
    fun half(n: Int): Int = n / 2
}

class LocalRename {
    fun sum(xs: List<Int>): Int {
        val total = xs.sum()
        return total
    }
}

class TrailingComma {
    fun pair(a: Int, b: Int): Int = a + b
}

class Suppressed {
    fun magic(): Int = 42
}
