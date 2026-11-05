package demo

interface Schedule {
    fun slots(day: Int): Int
}

interface HasSchedule {
    val schedule: Schedule
}
