// Kiểm dữ liệu `transcripts` trước khi deploy tính năng transcript theo phiên — CHỈ ĐỌC, không ghi gì.
//
//   mongosh "$MONGO_URI" --quiet scripts/transcripts-audit.js
//
// In một đối tượng JSON. Mọi bộ đếm trong `problems` bằng 0 thì dữ liệu tương thích code mới.
// Có số khác 0: dừng deploy, lập danh sách `_id` và xin duyệt cách xử lý riêng.

const STUCK_MS = 60 * 60 * 1000;
const t = db.getCollection("transcripts");

const byStatus = {};
t.aggregate([{ $group: { _id: "$status", n: { $sum: 1 } } }]).forEach((g) => {
	byStatus[String(g._id)] = g.n;
});

const missing = (field) => t.countDocuments({ [field]: { $exists: false } });

const problems = {
	// Thiếu `runId`: worker coi như lượt đã huỷ; code mới cũng không gắn được câu sửa.
	missingRunId: missing("runId"),
	missingModel: missing("model"),
	missingProgress: missing("progress"),
	missingUserId: missing("userId"),
	// `running` không cập nhật hơn 1 giờ: kẹt sau `claim_next`.
	stuckRunning: t.countDocuments({
		status: "running",
		updatedAt: { $lt: new Date(Date.now() - STUCK_MS) },
	}),
	idNotObjectId: t.countDocuments({ _id: { $not: { $type: "objectId" } } }),
	// Không còn file tương ứng (chỉ đếm, không xoá).
	orphanTranscripts: t
		.aggregate([
			{ $lookup: { from: "files", localField: "_id", foreignField: "_id", as: "f" } },
			{ $match: { f: { $size: 0 } } },
			{ $count: "n" },
		])
		.toArray()[0]?.n ?? 0,
};

const result = {
	db: db.getName(),
	at: new Date().toISOString(),
	total: t.countDocuments({}),
	byStatus,
	problems,
	transcriptDocsExists: db.getCollectionNames().includes("transcript_docs"),
	ok: Object.values(problems).every((n) => n === 0),
};
print(JSON.stringify(result, null, 2));
