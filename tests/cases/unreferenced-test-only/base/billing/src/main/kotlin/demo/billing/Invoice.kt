package demo.billing

class Invoice(private val lines: List<Long>) {
    fun total(): Long = lines.sum()

    fun debugDump(): String = lines.joinToString()
}

fun invoiceTotal(lines: List<Long>): Long = Invoice(lines).total()
