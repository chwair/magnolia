export function getRatingColor(rating) {
  // Retain original numeric mapping so it can be used when a direct hex is required
  if (rating >= 9) return '#5fedd8';  // Aqua green (9-10)
  if (rating >= 8) return '#6bdb8f';  // Green (8-9)
  if (rating >= 7) return '#f5d95a';  // Yellow (7-8)
  if (rating >= 6) return '#ffa368';  // Orange (6-7)
  if (rating >= 5) return '#ff6b6b';  // Red (5-6)
  return '#d65db1';  // Purplish red (0-5)
}

export function getRatingClass(rating) {
  // Map numeric rating to a semantic CSS class so styles stay in CSS rather than inline
  if (rating >= 9) return 'rating--aqua';
  if (rating >= 8) return 'rating--green';
  if (rating >= 7) return 'rating--yellow';
  if (rating >= 6) return 'rating--orange';
  if (rating >= 5) return 'rating--red';
  return 'rating--purple';
}

export const RATING_CLASSES = {
  'rating--aqua': '#5fedd8',
  'rating--green': '#6bdb8f',
  'rating--yellow': '#f5d95a',
  'rating--orange': '#ffa368',
  'rating--red': '#ff6b6b',
  'rating--purple': '#d65db1'
};
