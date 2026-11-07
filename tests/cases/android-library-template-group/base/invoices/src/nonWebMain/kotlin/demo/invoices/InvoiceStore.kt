package demo.invoices

class InvoiceStore {
    private val rows = mutableMapOf<String, Invoice>()

    fun put(row: Invoice) {
        rows[row.id] = row
    }

    fun total(): Long = rows.values.sumOf { it.totalCents }
}
