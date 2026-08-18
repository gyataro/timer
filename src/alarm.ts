import alarmUrl from "./assets/gentle-alarm.mp3";

const alarm = new Audio(alarmUrl);
alarm.preload = "auto";
alarm.volume = 0.9;

export function playAlarm(): void {
  alarm.currentTime = 0;
  void alarm.play().catch(() => {
    // Browsers/WebViews can reject playback until the user has interacted
    // with the app. The timer itself should continue regardless.
  });
}

