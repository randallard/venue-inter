type ToastType = 'info' | 'success' | 'error';

interface Toast {
	id: number;
	message: string;
	type: ToastType;
}

let nextId = 0;
let toasts = $state<Toast[]>([]);

export function getToasts() {
	return toasts;
}

export function addToast(message: string, type: ToastType = 'info', duration = 4000) {
	const id = nextId++;
	toasts.push({ id, message, type });
	setTimeout(() => {
		toasts = toasts.filter((t) => t.id !== id);
	}, duration);
}
