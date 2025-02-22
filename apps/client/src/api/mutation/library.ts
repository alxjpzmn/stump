import API from '..';

// TODO: type this
export function scanLibary(id: string): Promise<unknown> {
	return API.get(`/libraries/${id}/scan`);
}

// TODO: type this
export function deleteLibrary(id: string) {
	return API.delete(`/libraries/${id}`);
}

export function createLibrary(payload: CreateLibraryInput): Promise<ApiResult<Library>> {
	return API.post('/libraries', payload);
}

export function editLibrary(payload: EditLibraryInput): Promise<ApiResult<Library>> {
	return API.put(`/libraries/${payload.id}`, payload);
}
