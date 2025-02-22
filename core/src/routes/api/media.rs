use prisma_client_rust::{raw, Direction};
use rocket::{fs::NamedFile, serde::json::Json};
use rocket_okapi::openapi;

use crate::{
	fs,
	guards::auth::Auth,
	prisma::{media, read_progress, user},
	types::{
		alias::{ApiResult, Context},
		errors::ApiError,
		http::{FileResponse, ImageResponse},
		models::{media::Media, read_progress::ReadProgress},
		pageable::{Pageable, PagedRequestParams},
	},
};

// TODO: paginate some of these?

#[openapi(tag = "Media")]
#[get("/media?<unpaged>&<page_params..>")]
pub async fn get_media(
	unpaged: Option<bool>,
	page_params: Option<PagedRequestParams>,
	ctx: &Context,
	auth: Auth,
) -> ApiResult<Json<Pageable<Vec<Media>>>> {
	let db = ctx.get_db();

	let media = db
		.media()
		.find_many(vec![])
		.with(media::read_progresses::fetch(vec![
			read_progress::user_id::equals(auth.0.id),
		]))
		.order_by(media::name::order(Direction::Asc))
		.exec()
		.await?
		.into_iter()
		.map(|m| m.into())
		.collect::<Vec<Media>>();

	let unpaged = match unpaged {
		Some(val) => val,
		None => page_params.is_none(),
	};

	if unpaged {
		return Ok(Json(media.into()));
	}

	Ok(Json((media, page_params).into()))
}

#[openapi(tag = "Media")]
#[get("/media/duplicates?<unpaged>&<page_params..>")]
pub async fn get_duplicate_media(
	unpaged: Option<bool>,
	page_params: Option<PagedRequestParams>,
	ctx: &Context,
	_auth: Auth,
) -> ApiResult<Json<Pageable<Vec<Media>>>> {
	let db = ctx.get_db();

	let media: Vec<Media> = db
		._query_raw(raw!("SELECT * FROM media WHERE checksum IN (SELECT checksum FROM media GROUP BY checksum HAVING COUNT(*) > 1)"))
		.await?;

	let unpaged = match unpaged {
		Some(val) => val,
		None => page_params.is_none(),
	};

	if unpaged {
		return Ok(Json(media.into()));
	}

	Ok(Json((media, page_params).into()))
}

#[openapi(tag = "Media")]
#[get("/media/keep-reading")]
pub async fn get_reading_media(ctx: &Context, auth: Auth) -> ApiResult<Json<Vec<Media>>> {
	let db = ctx.get_db();

	Ok(Json(
		db.media()
			.find_many(vec![media::read_progresses::some(vec![
				read_progress::user_id::equals(auth.0.id.clone()),
				read_progress::page::gt(0),
			])])
			.with(media::read_progresses::fetch(vec![
				read_progress::user_id::equals(auth.0.id),
				read_progress::page::gt(0),
			]))
			.order_by(media::name::order(Direction::Asc))
			.exec()
			.await?
			.into_iter()
			.filter(|m| match m.read_progresses() {
				// Read progresses relation on media is one to many, there is a dual key
				// on read_progresses table linking a user and media. Therefore, there should
				// only be 1 item in this vec for each media resulting from the query.
				Ok(progress) => progress.len() == 1 && progress[0].page < m.pages,
				_ => false,
			})
			.map(|m| m.into())
			.collect(),
	))
}

#[openapi(tag = "Media")]
#[get("/media/<id>")]
pub async fn get_media_by_id(
	id: String,
	ctx: &Context,
	auth: Auth,
) -> ApiResult<Json<Media>> {
	let db = ctx.get_db();

	let book = db
		.media()
		.find_unique(media::id::equals(id.clone()))
		.with(media::read_progresses::fetch(vec![
			read_progress::user_id::equals(auth.0.id),
		]))
		.exec()
		.await?;

	if book.is_none() {
		return Err(ApiError::NotFound(format!(
			"Media with id {} not found",
			id
		)));
	}

	Ok(Json(book.unwrap().into()))
}

#[openapi(tag = "Media")]
#[get("/media/<id>/file")]
pub async fn get_media_file(
	id: String,
	ctx: &Context,
	// _auth: Auth,
) -> ApiResult<FileResponse> {
	let db = ctx.get_db();

	let media = db
		.media()
		.find_unique(media::id::equals(id.clone()))
		.exec()
		.await?;

	if media.is_none() {
		return Err(ApiError::NotFound(format!(
			"Media with id {} not found",
			id
		)));
	}

	let media = media.unwrap();

	Ok(FileResponse(
		NamedFile::open(media.path.clone()).await?,
		media.path,
	))

	// Ok(NamedFile::open(media.path.clone()).await?)
}

#[openapi(tag = "Media")]
#[post("/media/<id>/convert")]
pub async fn convert_media_to_cbz(
	id: String,
	ctx: &Context,
	_auth: Auth,
) -> Result<(), ApiError> {
	let db = ctx.get_db();

	let media = db
		.media()
		.find_unique(media::id::equals(id.clone()))
		.exec()
		.await?;

	if media.is_none() {
		return Err(ApiError::NotFound(format!(
			"Media with id {} not found",
			id
		)));
	}

	let media = media.unwrap();

	if media.extension != "cbr" {
		return Err(ApiError::BadRequest(format!(
			"Media with id {} is not a cbr file. Only cbr files can be converted to cbz",
			id
		)));
	}

	unimplemented!()
}

#[openapi(tag = "Media")]
#[get("/media/<id>/page/<page>")]
pub async fn get_media_page(
	id: String,
	page: i32,
	ctx: &Context,
	auth: Auth,
) -> ApiResult<ImageResponse> {
	let db = ctx.get_db();

	let book = db
		.media()
		.find_unique(media::id::equals(id.clone()))
		.with(media::read_progresses::fetch(vec![
			read_progress::user_id::equals(auth.0.id),
		]))
		.exec()
		.await?;

	match book {
		Some(book) => {
			if page > book.pages {
				// FIXME: probably won't work lol
				Err(ApiError::Redirect(format!(
					"/book/{}/read?page={}",
					id, book.pages
				)))
			} else {
				Ok(fs::media_file::get_page(&book.path, page)?)
			}
		},
		None => Err(ApiError::NotFound(format!(
			"Media with id {} not found",
			id
		))),
	}
}

#[openapi(tag = "Media")]
#[get("/media/<id>/thumbnail")]
pub async fn get_media_thumbnail(
	id: String,
	ctx: &Context,
	auth: Auth,
) -> ApiResult<ImageResponse> {
	let db = ctx.get_db();

	let book = db
		.media()
		.find_unique(media::id::equals(id.clone()))
		.with(media::read_progresses::fetch(vec![
			read_progress::user_id::equals(auth.0.id),
		]))
		.exec()
		.await?;

	if book.is_none() {
		return Err(ApiError::NotFound(format!(
			"Media with id {} not found",
			id
		)));
	}

	let book = book.unwrap();

	Ok(fs::media_file::get_page(book.path.as_str(), 1)?)
}

// FIXME: this doesn't really handle certain errors correctly, e.g. media/user not found
#[openapi(tag = "Media")]
#[put("/media/<id>/progress/<page>")]
pub async fn update_media_progress(
	id: String,
	page: i32,
	ctx: &Context,
	auth: Auth,
) -> ApiResult<Json<ReadProgress>> {
	let db = ctx.get_db();

	// update the progress, otherwise create it
	Ok(Json(
		db.read_progress()
			.upsert(
				read_progress::UniqueWhereParam::UserIdMediaIdEquals(
					auth.0.id.clone(),
					id.clone(),
				),
				(
					read_progress::page::set(page),
					read_progress::media::link(media::id::equals(id.clone())),
					read_progress::user::link(user::id::equals(auth.0.id.clone())),
					vec![],
				),
				vec![read_progress::page::set(page)],
			)
			.exec()
			.await?
			.into(),
	))
}
