package demo.orders

class OrderStore {
    private val rows = mutableMapOf<String, Order>()

    fun put(row: Order) {
        rows[row.id] = row
    }

    fun total(): Long = rows.values.sumOf { it.totalCents }.coerceAtLeast(0)
}
