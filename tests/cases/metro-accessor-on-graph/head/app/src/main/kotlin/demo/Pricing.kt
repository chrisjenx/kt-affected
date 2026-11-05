package demo

import dev.zacsweers.metro.AppScope
import dev.zacsweers.metro.ContributesBinding
import dev.zacsweers.metro.Inject

interface Pricing {
    fun price(sku: String): Int
    fun currency(): String
}

@ContributesBinding(AppScope::class)
@Inject
class PricingImpl : Pricing {
    override fun price(sku: String): Int = sku.length * 125
    override fun currency(): String = "USD"
}
