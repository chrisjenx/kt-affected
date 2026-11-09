package demo.shelves

import kotlin.test.Test as KTest
import org.junit.jupiter.params.ParameterizedTest
import org.junit.jupiter.params.provider.EnumSource
import org.junit.jupiter.params.provider.MethodSource

class RouteTest {
    @KTest
    fun `slot is built`() {
        check(slot() is Slot)
    }

    @ParameterizedTest
    @EnumSource(Route::class)
    fun everyRouteIsAuthorised(route: Route) {
        check(route.name.isNotEmpty())
    }

    @ParameterizedTest
    @MethodSource("widths")
    fun widthsArePositive(w: Int) {
        check(w > 0)
    }

    companion object {
        @JvmStatic
        fun widths() = listOf(1, 2)
    }
}
