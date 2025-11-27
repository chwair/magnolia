const TMDB_BASE_URL = 'https://api.themoviedb.org/3';
const TMDB_IMAGE_BASE_URL = 'https://image.tmdb.org/t/p';
const TMDB_READ_ACCESS_TOKEN = 'eyJhbGciOiJIUzI1NiJ9.eyJhdWQiOiIyNzI0YTQ3YWJlY2IyNWM5ZGMyNWZmMzFiMmI3YmM5OCIsIm5iZiI6MTc2MjYyMDQxNi4xNSwic3ViIjoiNjkwZjc0MDA5NjViMzViYjIzOWFiYTA3Iiwic2NvcGVzIjpbImFwaV9yZWFkIl0sInZlcnNpb24iOjF9.esGuObRVhX3ZsXFA-iqWYZcZiFNdRiJYQDuWzpLAYyI';

const headers = {
  'Authorization': `Bearer ${TMDB_READ_ACCESS_TOKEN}`,
  'Content-Type': 'application/json;charset=utf-8'
};

export function getImageUrl(path, size = 'w500') {
  if (!path) return null;
  return `${TMDB_IMAGE_BASE_URL}/${size}${path}`;
}

export async function getConfiguration() {
  const response = await fetch(`${TMDB_BASE_URL}/configuration`, { headers });
  return response.json();
}

export async function getTrending(mediaType = 'all', timeWindow = 'day', page = 1) {
  const response = await fetch(
    `${TMDB_BASE_URL}/trending/${mediaType}/${timeWindow}?page=${page}`,
    { headers }
  );
  return response.json();
}

export async function getPopularMovies(page = 1) {
  const response = await fetch(
    `${TMDB_BASE_URL}/movie/popular?page=${page}`,
    { headers }
  );
  return response.json();
}

export async function getPopularTV(page = 1) {
  const response = await fetch(
    `${TMDB_BASE_URL}/tv/popular?page=${page}`,
    { headers }
  );
  return response.json();
}

export async function getTopRatedMovies(page = 1) {
  const response = await fetch(
    `${TMDB_BASE_URL}/movie/top_rated?page=${page}`,
    { headers }
  );
  return response.json();
}

export async function getTopRatedTV(page = 1) {
  const response = await fetch(
    `${TMDB_BASE_URL}/tv/top_rated?page=${page}`,
    { headers }
  );
  return response.json();
}

export async function getNowPlaying(page = 1) {
  const response = await fetch(
    `${TMDB_BASE_URL}/movie/now_playing?page=${page}`,
    { headers }
  );
  return response.json();
}

export async function discoverMovies(params = {}) {
  const queryString = new URLSearchParams(params).toString();
  const response = await fetch(
    `${TMDB_BASE_URL}/discover/movie?${queryString}`,
    { headers }
  );
  return response.json();
}

export async function discoverTV(params = {}) {
  const queryString = new URLSearchParams(params).toString();
  const response = await fetch(
    `${TMDB_BASE_URL}/discover/tv?${queryString}`,
    { headers }
  );
  return response.json();
}

export async function getMovieDetails(movieId) {
  const response = await fetch(
    `${TMDB_BASE_URL}/movie/${movieId}?append_to_response=credits,videos,images,similar`,
    { headers }
  );
  return response.json();
}

export async function getTVDetails(tvId) {
  const response = await fetch(
    `${TMDB_BASE_URL}/tv/${tvId}?append_to_response=credits,videos,images,similar,aggregate_credits`,
    { headers }
  );
  return response.json();
}

export async function getSeasonDetails(tvId, seasonNumber) {
  const response = await fetch(
    `${TMDB_BASE_URL}/tv/${tvId}/season/${seasonNumber}`,
    { headers }
  );
  return response.json();
}

export async function getEpisodeDetails(tvId, seasonNumber, episodeNumber) {
  const response = await fetch(
    `${TMDB_BASE_URL}/tv/${tvId}/season/${seasonNumber}/episode/${episodeNumber}`,
    { headers }
  );
  return response.json();
}

export async function searchMulti(query, page = 1) {
  const response = await fetch(
    `${TMDB_BASE_URL}/search/multi?query=${encodeURIComponent(query)}&page=${page}`,
    { headers }
  );
  return response.json();
}

export async function searchMovies(query, page = 1) {
  const response = await fetch(
    `${TMDB_BASE_URL}/search/movie?query=${encodeURIComponent(query)}&page=${page}`,
    { headers }
  );
  return response.json();
}

export async function searchTV(query, page = 1) {
  const response = await fetch(
    `${TMDB_BASE_URL}/search/tv?query=${encodeURIComponent(query)}&page=${page}`,
    { headers }
  );
  return response.json();
}

export async function getMovieGenres() {
  const response = await fetch(
    `${TMDB_BASE_URL}/genre/movie/list`,
    { headers }
  );
  return response.json();
}

export async function getTVGenres() {
  const response = await fetch(
    `${TMDB_BASE_URL}/genre/tv/list`,
    { headers }
  );
  return response.json();
}

export async function getMovieCredits(movieId) {
  const response = await fetch(
    `${TMDB_BASE_URL}/movie/${movieId}/credits`,
    { headers }
  );
  return response.json();
}

export async function getTVCredits(tvId) {
  const response = await fetch(
    `${TMDB_BASE_URL}/tv/${tvId}/credits`,
    { headers }
  );
  return response.json();
}

export async function getMovieRecommendations(movieId, page = 1) {
  const response = await fetch(
    `${TMDB_BASE_URL}/movie/${movieId}/recommendations?page=${page}`,
    { headers }
  );
  return response.json();
}

export async function getTVRecommendations(tvId, page = 1) {
  const response = await fetch(
    `${TMDB_BASE_URL}/tv/${tvId}/recommendations?page=${page}`,
    { headers }
  );
  return response.json();
}

export async function getSimilarMovies(movieId, page = 1) {
  const response = await fetch(
    `${TMDB_BASE_URL}/movie/${movieId}/similar?page=${page}`,
    { headers }
  );
  return response.json();
}

export async function getSimilarTV(tvId, page = 1) {
  const response = await fetch(
    `${TMDB_BASE_URL}/tv/${tvId}/similar?page=${page}`,
    { headers }
  );
  return response.json();
}

export async function getMovieExternalIds(movieId) {
  const response = await fetch(
    `${TMDB_BASE_URL}/movie/${movieId}/external_ids`,
    { headers }
  );
  return response.json();
}

export async function getTVExternalIds(tvId) {
  const response = await fetch(
    `${TMDB_BASE_URL}/tv/${tvId}/external_ids`,
    { headers }
  );
  return response.json();
}

export async function getEpisodeExternalIds(tvId, seasonNumber, episodeNumber) {
  const response = await fetch(
    `${TMDB_BASE_URL}/tv/${tvId}/season/${seasonNumber}/episode/${episodeNumber}/external_ids`,
    { headers }
  );
  return response.json();
}
