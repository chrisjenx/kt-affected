package demo

import dev.zacsweers.metro.AppScope
import dev.zacsweers.metro.ContributesBinding
import dev.zacsweers.metro.Inject

interface Rates {
    fun perChar(): Int
}

@ContributesBinding(AppScope::class)
@Inject
class RatesImpl : Rates {
    override fun perChar(): Int = 125
}

interface Pricing {
    fun price(sku: String): Int
    fun currency(): String
}

interface HasPricing {
    val pricing: Pricing
}

@ContributesBinding(AppScope::class)
@Inject
class PricingImpl(private val rates: Rates) : Pricing {
    override fun price(sku: String): Int = sku.length * rates.perChar()
    override fun currency(): String = "USD"
}
