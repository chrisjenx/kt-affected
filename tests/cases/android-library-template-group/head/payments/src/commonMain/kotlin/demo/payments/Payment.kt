package demo.payments

data class Payment(val id: String, val totalCents: Long) {
    fun label(): String = "payment #" + id
}
