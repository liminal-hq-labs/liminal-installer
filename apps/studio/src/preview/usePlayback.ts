// A clock for the copy animation: plays at a chosen speed, pauses and resets.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useCallback, useEffect, useRef, useState } from 'react';

export interface Playback {
	/** Seconds played at the chosen speed. */
	t: number;
	playing: boolean;
	speed: number;
	play(): void;
	pause(): void;
	reset(): void;
	setSpeed(speed: number): void;
}

export function usePlayback(): Playback {
	const [t, setT] = useState(0);
	const [playing, setPlaying] = useState(false);
	const [speed, setSpeed] = useState(1);
	const speedRef = useRef(speed);
	speedRef.current = speed;
	useEffect(() => {
		if (!playing) return;
		let last = performance.now();
		let frame = requestAnimationFrame(function tick(now) {
			setT((v) => v + ((now - last) / 1000) * speedRef.current);
			last = now;
			frame = requestAnimationFrame(tick);
		});
		return () => cancelAnimationFrame(frame);
	}, [playing]);
	const reset = useCallback(() => {
		setPlaying(false);
		setT(0);
	}, []);
	return {
		t,
		playing,
		speed,
		play: () => setPlaying(true),
		pause: () => setPlaying(false),
		reset,
		setSpeed,
	};
}
