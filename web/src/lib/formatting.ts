export function shortAddress(
  value: string,
  leading = 6,
  trailing = 4,
) {
  if (value.length <= leading + trailing + 2) {
    return value;
  }

  return `${value.slice(0, leading)}…${value.slice(-trailing)}`;
}

export function formatHexQuantity(value: string) {
  try {
    return groupNumericString(BigInt(value).toString());
  } catch {
    return value;
  }
}

export function formatJson(value: unknown) {
  return JSON.stringify(value, null, 2);
}

export function formatTimestampLabel(iso: string) {
  const diff = Date.now() - new Date(iso).getTime();
  const minutes = Math.floor(diff / 60_000);

  if (minutes < 1) {
    return 'just now';
  }

  if (minutes < 60) {
    return `${minutes}m ago`;
  }

  const hours = Math.floor(minutes / 60);

  if (hours < 24) {
    return `${hours}h ago`;
  }

  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

function groupNumericString(value: string) {
  const negative = value.startsWith('-');
  const digits = negative ? value.slice(1) : value;
  const grouped = digits.replace(/\B(?=(\d{3})+(?!\d))/g, ',');

  return negative ? `-${grouped}` : grouped;
}
