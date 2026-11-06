package demo.app

class Invoice(val lines: List<Int>) {
    fun total(): Int = lines.sum() + 1
}
