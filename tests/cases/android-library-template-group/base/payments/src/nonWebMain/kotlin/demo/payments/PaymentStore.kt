package demo.payments

class PaymentStore {
    private val rows = mutableMapOf<String, Payment>()

    fun put(row: Payment) {
        rows[row.id] = row
    }

    fun total(): Long = rows.values.sumOf { it.totalCents }
}
