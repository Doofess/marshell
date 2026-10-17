import { useEffect, useRef, useState, type HTMLAttributes } from "react";
import { createAnnouncer, type Announcer as AnnouncerInstance, type Message } from "./announce-policy";

/** One announcer for the lifetime of a component. Create it once per window and pass it to whatever needs to say something. */
export function useAnnouncer(): { announcer: AnnouncerInstance; message: Message } {
  const ref = useRef<AnnouncerInstance>(null);
  ref.current ??= createAnnouncer();
  const announcer = ref.current;
  const [message, setMessage] = useState<Message>(announcer.current());
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const arm = () => {
      clearTimeout(timer);
      const delay = announcer.pendingDelay();
      if (delay !== null) timer = setTimeout(() => announcer.flush(), delay);
    };
    const off = announcer.subscribe((m) => setMessage(m));
    // A message held for the rate limit is said when its interval is over; the check runs after every say.
    const say = announcer.say;
    announcer.say = (text, opts) => {
      say(text, opts);
      arm();
    };
    return () => {
      off();
      clearTimeout(timer);
      announcer.say = say;
    };
  }, [announcer]);
  return { announcer, message };
}

/**
 * THE live region of the app: the only element in src/** allowed to be a live region (sharedBlocks.test.ts). It renders
 * empty until something is said, so nothing is read out as a window opens. Visually hidden by the caller's class.
 */
export function Announcer({ message, className, ...rest }: { message: Message } & HTMLAttributes<HTMLDivElement>) {
  return (
    <div className={className} role="status" aria-live={message.priority} aria-atomic="true" {...rest}>
      {/* A new node for each message: the same words said again must still change the region, or it is not read. */}
      {message.text && <span key={message.seq}>{message.text}</span>}
    </div>
  );
}
