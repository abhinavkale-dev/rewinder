"use client";

import { motion } from "framer-motion";
import { useEffect, useRef, useState } from "react";

const ClockAnimation = () => {
  const [time, setTime] = useState(() => new Date());
  const requestRef = useRef<number | null>(null);

  useEffect(() => {
    const update = () => {
      setTime(new Date());
      requestRef.current = requestAnimationFrame(update);
    };

    requestRef.current = requestAnimationFrame(update);

    return () => {
      if (requestRef.current) {
        cancelAnimationFrame(requestRef.current);
      }
    };
  }, []);

  const seconds = time.getSeconds() + time.getMilliseconds() / 1000;
  const minutes = time.getMinutes() + seconds / 60;
  const hours = (time.getHours() % 12) + minutes / 60;
  const secondAngle = (seconds / 60) * 360;
  const minuteAngle = (minutes / 60) * 360;
  const hourAngle = (hours / 12) * 360;

  return (
    <svg
      className="invert dark:invert-0"
      xmlns="http://www.w3.org/2000/svg"
      width="277"
      height="277"
      fill="none"
      viewBox="0 0 277 277"
    >
      <g className="Group 2085662490">
        <circle
          cx="138.064"
          cy="138.063"
          r="106.205"
          stroke="#fff"
          className="Ellipse 43489"
        />
        <circle
          cx="138.064"
          cy="138.063"
          r="103.559"
          stroke="#fff"
          strokeDasharray="1 20"
          strokeWidth="2"
          className="Ellipse 43490"
        />
        <motion.g
          animate={{ rotate: secondAngle }}
          transition={{ type: "tween", ease: "linear", duration: 0.1 }}
          style={{ transformOrigin: "50% 50%" }}
          className="Group 2085662488 rotate-45 second"
        >
          <circle
            cx="138.064"
            cy="138.063"
            r="106.205"
            stroke="#fff"
            className="Ellipse 43491"
          />
          <path
            stroke="#fff"
            d="M137.564 138.063V51.422"
            className="Line 214"
          />
        </motion.g>

        <motion.g
          animate={{ rotate: minuteAngle }}
          transition={{ type: "tween", ease: "linear", duration: 0.1 }}
          style={{ transformOrigin: "50% 50%" }}
          className="Group 2085662489 minute"
        >
          <circle
            cx="138.064"
            cy="138.064"
            r="106.205"
            stroke="#fff"
            className="Ellipse 43493"
            transform="rotate(-21.194 138.064 138.064)"
            opacity="0.2"
          />
          <path
            stroke="#fff"
            strokeWidth="2"
            d="m137.564 138.064.001-65"
            className="Line 216"
          />
        </motion.g>

        <motion.g
          animate={{ rotate: hourAngle }}
          transition={{ type: "tween", ease: "linear", duration: 0.1 }}
          style={{ transformOrigin: "50% 50%" }}
          className="Group 2085662489 hour"
        >
          <circle
            cx="138.064"
            cy="138.064"
            r="106.205"
            stroke="#fff"
            className="Ellipse 43492"
            transform="rotate(-21.194 138.064 138.064)"
          />
          <path
            stroke="#fff"
            d="m137.564 138.064.001-49.232"
            className="Line 215"
          />
        </motion.g>

        <motion.g
          animate={{ rotate: hourAngle }}
          transition={{ type: "tween", ease: "linear", duration: 0.1 }}
          style={{ transformOrigin: "50% 50%" }}
          className="Group 2085662489 hour"
        >
          <circle
            cx="138.064"
            cy="138.064"
            r="106.205"
            stroke="#fff"
            className="Ellipse 43492"
            transform="rotate(-21.194 138.064 138.064)"
          />
          <path
            stroke="#fff"
            d="m137.564 138.064.001-49.232"
            className="Line 215"
          />
        </motion.g>

        <rect
          width="10.851"
          height="10.851"
          x="132.639"
          y="132.638"
          fill="#D9D9D9"
          className="Rectangle 240648367"
          rx="5.425"
        />
      </g>
    </svg>
  );
};

export default ClockAnimation;





