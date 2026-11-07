package demo.invoices

data class Invoice(val id: String, val totalCents: Long) {
    fun label(): String = "invoice #" + id
}
