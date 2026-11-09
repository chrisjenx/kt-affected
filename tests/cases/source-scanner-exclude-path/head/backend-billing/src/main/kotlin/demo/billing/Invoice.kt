package demo.billing

data class Invoice(val id: String, val amount: Long, val paid: Boolean = false)
