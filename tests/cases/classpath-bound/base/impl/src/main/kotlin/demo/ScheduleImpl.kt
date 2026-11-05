package demo

@ContributesBinding(AppScope::class)
class ScheduleImpl : Schedule {
    override fun slots(day: Int): Int = perDay(day)

    private fun perDay(day: Int): Int = day * 4
}
