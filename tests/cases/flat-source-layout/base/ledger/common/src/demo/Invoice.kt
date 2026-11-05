package demo

class Invoice(private val lines: List<Int>) {
    fun total(): Int = lines.sum()
}
