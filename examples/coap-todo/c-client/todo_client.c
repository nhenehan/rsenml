/*
 * todo-c: the C todo client. libcoap speaks CoAP, exip reads and writes SenML
 * EXI.
 *
 *   todo-c [--host H] [--port N] list | add <title> | done <id> | remove <id>
 *
 * SenML EXI is the XML form of SenML (RFC 8428 section 7) coded as strict,
 * schema-informed EXI, with the EXI options in the header and schemaId "a"
 * (https://www.rfc-editor.org/rfc/rfc8428#section-8). exip loads the SenML
 * schema at start-up, from the schema's own EXI form (schemas/senml.xsd.exi).
 */
#include <coap3/coap.h>

#include <netdb.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "EXIParser.h"
#include "EXISerializer.h"
#include "grammarGenerator.h"
#include "stringManipulate.h"

#define SENML_EXI                                                              \
  114 /* CoAP Content-Format of application/senml-exi (RFC 8428 section 12.3)  \
       */
#define MAX_ITEMS 256
#define MAX_TEXT 512

static const char BASE_NAME[] = "urn:example:todo:";
static String SENML_NS = {"urn:ietf:params:xml:ns:senml", 28};
static String EMPTY = {NULL, 0};
static String LN_SENSML = {"sensml", 6};
static String LN_SENML = {"senml", 5};
static String LN_BN = {"bn", 2};
static String LN_N = {"n", 1};
static String LN_VB = {"vb", 2};
static String LN_VS = {"vs", 2};

static void die(const char *what, const char *detail) {
  fprintf(stderr, "todo-c: %s%s%s\n", what, detail ? ": " : "",
          detail ? detail : "");
  exit(1);
}

/* ---- the SenML schema ---------------------------------------------------- */

static EXIPSchema senml_schema;

static void load_schema(void) {
  FILE *f = fopen(TODO_SENML_XSD_EXI, "rb");
  if (!f)
    die("cannot open " TODO_SENML_XSD_EXI, NULL);
  static char data[4096];
  size_t n = fread(data, 1, sizeof data, f);
  fclose(f);
  BinaryBuffer buf = {data, sizeof data, n, {NULL, NULL}};
  if (generateSchemaInformedGrammars(&buf, 1, SCHEMA_FORMAT_XSD_EXI, NULL,
                                     &senml_schema, NULL) != EXIP_OK)
    die("exip could not load the SenML schema", NULL);
}

/* ---- writing a one-record pack ------------------------------------------- */

typedef struct {
  unsigned char data[1024];
  size_t len;
} Bytes;

static size_t bytes_write(void *buf, size_t size, void *stream) {
  Bytes *b = stream;
  if (b->len + size > sizeof b->data)
    return 0;
  memcpy(b->data + b->len, buf, size);
  b->len += size;
  return size;
}

#define CHECK(call)                                                            \
  do {                                                                         \
    if ((call) != EXIP_OK)                                                     \
      die("exip could not encode the pack", #call);                            \
  } while (0)

/*
 * <sensml><senml bn="urn:example:todo:" n="FIELD" vs="TEXT"/></sensml>, or vb
 * when text is NULL. In strict EXI the attributes must come in the order the
 * grammar lists them: by local name, then namespace, so bn, n, then the value
 * (EXI 8.5.4.1.3.2, https://www.w3.org/TR/exi/#complexTypeGrammars).
 */
static void encode_field(const char *field, const char *text, int flag,
                         Bytes *out) {
  EXIStream strm;
  char chunk[256];
  EXITypeClass type;
  BinaryBuffer buffer = {chunk, sizeof chunk, 0, {bytes_write, out}};
  QName q = {&SENML_NS, &LN_SENSML, NULL};
  String bn = {(char *)BASE_NAME, sizeof BASE_NAME - 1};
  String n = {(char *)field, strlen(field)};

  /* exip writes one byte past the end when the stream ends on a byte edge
   * (rexi-interop finding); zero it so the extra byte is at least the same. */
  memset(chunk, 0, sizeof chunk);
  out->len = 0;
  serialize.initHeader(&strm);
  strm.header.has_cookie = FALSE;
  strm.header.has_options =
      TRUE; /* RFC 8428 section 8: options in the header */
  SET_STRICT(strm.header.opts.enumOpt);
  strm.header.opts.schemaIDMode = SCHEMA_ID_SET;
  strm.header.opts.schemaID.str = "a";
  strm.header.opts.schemaID.length = 1;
  CHECK(serialize.initStream(&strm, buffer, &senml_schema));
  CHECK(serialize.exiHeader(&strm));
  CHECK(serialize.startDocument(&strm));
  CHECK(serialize.startElement(&strm, q, &type));
  q.localName = &LN_SENML;
  CHECK(serialize.startElement(&strm, q, &type));
  q.uri = &EMPTY;
  q.localName = &LN_BN;
  CHECK(serialize.attribute(&strm, q, TRUE, &type));
  CHECK(serialize.stringData(&strm, bn));
  q.localName = &LN_N;
  CHECK(serialize.attribute(&strm, q, TRUE, &type));
  CHECK(serialize.stringData(&strm, n));
  if (text) {
    String vs = {(char *)text, strlen(text)};
    q.localName = &LN_VS;
    CHECK(serialize.attribute(&strm, q, TRUE, &type));
    CHECK(serialize.stringData(&strm, vs));
  } else {
    q.localName = &LN_VB;
    CHECK(serialize.attribute(&strm, q, TRUE, &type));
    CHECK(serialize.booleanData(&strm, flag ? TRUE : FALSE));
  }
  CHECK(serialize.endElement(&strm));
  CHECK(serialize.endElement(&strm));
  CHECK(serialize.endDocument(&strm));
  CHECK(serialize.closeEXIStream(&strm));
}

/* ---- reading the list pack ------------------------------------------------
 */

typedef struct {
  int used;
  char title[MAX_TEXT];
  int done;
} Item;

typedef struct {
  Item items[MAX_ITEMS + 1]; /* by id; ids above MAX_ITEMS are not shown */
  char base_name[MAX_TEXT];
  /* the record being read */
  char bn[MAX_TEXT], n[MAX_TEXT], vs[MAX_TEXT];
  int in_record; /* inside a <senml> element */
  int has_bn, has_vs, has_vb, vb;
  const String
      *pending; /* local name of the attribute whose value comes next */
} List;

static void copy_string(char *dst, String s) {
  size_t n = s.length < MAX_TEXT - 1 ? s.length : MAX_TEXT - 1;
  memcpy(dst, s.str, n);
  dst[n] = '\0';
}

static int is(const String *a, const String *b) {
  return a->length == b->length && memcmp(a->str, b->str, a->length) == 0;
}

static errorCode on_start(QName q, void *app) {
  List *l = app;
  if (is(q.localName, &LN_SENML)) {
    l->n[0] = '\0';
    l->has_bn = l->has_vs = l->has_vb = 0;
    l->in_record = 1;
  }
  return EXIP_OK;
}

static errorCode on_attribute(QName q, void *app) {
  List *l = app;
  l->pending = q.localName;
  return EXIP_OK;
}

static errorCode on_string(const String s, void *app) {
  List *l = app;
  if (!l->pending)
    return EXIP_OK;
  if (is(l->pending, &LN_BN)) {
    copy_string(l->bn, s);
    l->has_bn = 1;
  } else if (is(l->pending, &LN_N)) {
    copy_string(l->n, s);
  } else if (is(l->pending, &LN_VS)) {
    copy_string(l->vs, s);
    l->has_vs = 1;
  }
  l->pending = NULL;
  return EXIP_OK;
}

static errorCode on_boolean(exip_bool b, void *app) {
  List *l = app;
  if (l->pending && is(l->pending, &LN_VB)) {
    l->vb = b == TRUE;
    l->has_vb = 1;
  }
  l->pending = NULL;
  return EXIP_OK;
}

static errorCode on_float(Float f, void *app) {
  (void)f; /* the count record's v; the items say it all */
  ((List *)app)->pending = NULL;
  return EXIP_OK;
}

/*
 * End of a <senml> record: its full name is the base name in force plus its own
 * name (https://www.rfc-editor.org/rfc/rfc8428#section-4.5.1). Item records are
 * named <id>/title and <id>/done.
 */
static errorCode on_end(void *app) {
  List *l = app;
  char name[2 * MAX_TEXT];
  if (!l->in_record)
    return EXIP_OK; /* the end of <sensml> */
  l->in_record = 0;
  if (l->has_bn) {
    strcpy(l->base_name, l->bn);
    l->has_bn = 0;
  }
  snprintf(name, sizeof name, "%s%s", l->base_name, l->n);
  if (strncmp(name, BASE_NAME, sizeof BASE_NAME - 1) != 0)
    return EXIP_OK;
  char *rest = name + sizeof BASE_NAME - 1, *slash = strchr(rest, '/');
  char *end;
  long id = strtol(rest, &end, 10);
  if (!slash || end != slash || id < 1 || id > MAX_ITEMS)
    return EXIP_OK; /* the count record, or an id too big for this table */
  Item *it = &l->items[id];
  if (strcmp(slash + 1, "title") == 0 && l->has_vs) {
    it->used = 1;
    strcpy(it->title, l->vs);
  } else if (strcmp(slash + 1, "done") == 0 && l->has_vb) {
    it->used = 1;
    it->done = l->vb;
  }
  return EXIP_OK;
}

static errorCode on_error(const errorCode code, const char *msg, void *app) {
  (void)app;
  fprintf(stderr, "todo-c: exip error %d: %s\n", (int)code, msg);
  return EXIP_HANDLER_STOP;
}

static void print_list(const uint8_t *data, size_t len) {
  static List list;
  static char copy[4096];
  Parser parser;
  errorCode err;
  if (len > sizeof copy)
    die("the list is too long for this client", NULL);
  memcpy(copy, data, len);
  BinaryBuffer buf = {copy, sizeof copy, len, {NULL, NULL}};
  memset(&list, 0, sizeof list);

  if (parse.initParser(&parser, buf, &list) != EXIP_OK)
    die("exip could not start the parser", NULL);
  parser.handler.fatalError = on_error;
  parser.handler.error = on_error;
  parser.handler.startElement = on_start;
  parser.handler.endElement = on_end;
  parser.handler.attribute = on_attribute;
  parser.handler.stringData = on_string;
  parser.handler.booleanData = on_boolean;
  parser.handler.floatData = on_float;
  /* FALSE: the options come from the stream's header, not from us */
  if (parse.parseHeader(&parser, FALSE) != EXIP_OK)
    die("exip could not read the EXI header", NULL);
  if (parse.setSchema(&parser, &senml_schema) != EXIP_OK)
    die("exip could not use the SenML schema", NULL);
  while ((err = parse.parseNext(&parser)) == EXIP_OK) {
  }
  parse.destroyParser(&parser);
  if (err != EXIP_PARSING_COMPLETE)
    die("exip could not read the list", NULL);
  for (int id = 1; id <= MAX_ITEMS; id++) {
    if (list.items[id].used)
      printf("%d [%c] %s\n", id, list.items[id].done ? 'x' : ' ',
             list.items[id].title);
  }
}

/* ---- CoAP -----------------------------------------------------------------
 */

typedef struct {
  int finished;
  coap_pdu_code_t code;
  char location[64];
  uint8_t payload[4096];
  size_t payload_len;
  const char *failure;
} Exchange;

static Exchange exchange;

static coap_response_t on_response(coap_session_t *s, const coap_pdu_t *sent,
                                   const coap_pdu_t *received,
                                   const coap_mid_t mid) {
  (void)s;
  (void)sent;
  (void)mid;
  size_t len;
  const uint8_t *data;
  exchange.code = coap_pdu_get_code(received);
  if (coap_get_data(received, &len, &data) && len <= sizeof exchange.payload) {
    memcpy(exchange.payload, data, len);
    exchange.payload_len = len;
  }
  /* The last Location-Path segment is the new item's id. */
  coap_opt_iterator_t it;
  coap_opt_filter_t filter;
  coap_opt_t *opt;
  coap_option_filter_clear(&filter);
  coap_option_filter_set(&filter, COAP_OPTION_LOCATION_PATH);
  coap_option_iterator_init(received, &it, &filter);
  while ((opt = coap_option_next(&it)) != NULL) {
    size_t n = coap_opt_length(opt) < sizeof exchange.location - 1
                   ? coap_opt_length(opt)
                   : sizeof exchange.location - 1;
    memcpy(exchange.location, coap_opt_value(opt), n);
    exchange.location[n] = '\0';
  }
  exchange.finished = 1;
  return COAP_RESPONSE_OK;
}

static void on_nack(coap_session_t *s, const coap_pdu_t *sent,
                    const coap_nack_reason_t reason, const coap_mid_t mid) {
  (void)s;
  (void)sent;
  (void)mid;
  exchange.failure = reason == COAP_NACK_TOO_MANY_RETRIES
                         ? "no answer from the server"
                         : "request failed";
  exchange.finished = 1;
}

/*
 * One Confirmable request; libcoap sends it again when no ACK comes, as RFC
 * 7252 section 4.2 says (https://www.rfc-editor.org/rfc/rfc7252#section-4.2).
 */
static void request(const char *host, const char *port, coap_pdu_code_t method,
                    const char *id, const Bytes *body) {
  struct addrinfo hints = {0}, *ai;
  hints.ai_family = AF_UNSPEC;
  hints.ai_socktype = SOCK_DGRAM;
  if (getaddrinfo(host, port, &hints, &ai) != 0)
    die("cannot resolve", host);
  coap_address_t dst;
  coap_address_init(&dst);
  memcpy(&dst.addr.sa, ai->ai_addr, ai->ai_addrlen);
  dst.size = ai->ai_addrlen;
  freeaddrinfo(ai);

  coap_context_t *ctx = coap_new_context(NULL);
  coap_session_t *session =
      ctx ? coap_new_client_session(ctx, NULL, &dst, COAP_PROTO_UDP) : NULL;
  if (!session)
    die("cannot open a CoAP session", NULL);
  coap_register_response_handler(ctx, on_response);
  coap_register_nack_handler(ctx, on_nack);

  coap_pdu_t *pdu =
      coap_pdu_init(COAP_MESSAGE_CON, method, coap_new_message_id(session),
                    coap_session_max_pdu_size(session));
  uint8_t token[8];
  size_t token_len;
  coap_session_new_token(session, &token_len, token);
  coap_add_token(pdu, token_len, token);

  coap_optlist_t *options = NULL;
  uint8_t cf[4];
  coap_insert_optlist(&options, coap_new_optlist(COAP_OPTION_URI_PATH, 5,
                                                 (const uint8_t *)"todos"));
  if (id)
    coap_insert_optlist(&options,
                        coap_new_optlist(COAP_OPTION_URI_PATH, strlen(id),
                                         (const uint8_t *)id));
  if (body)
    coap_insert_optlist(
        &options,
        coap_new_optlist(COAP_OPTION_CONTENT_FORMAT,
                         coap_encode_var_safe(cf, sizeof cf, SENML_EXI), cf));
  coap_insert_optlist(
      &options,
      coap_new_optlist(COAP_OPTION_ACCEPT,
                       coap_encode_var_safe(cf, sizeof cf, SENML_EXI), cf));
  coap_add_optlist_pdu(pdu, &options);
  coap_delete_optlist(options);
  if (body)
    coap_add_data(pdu, body->len, body->data);

  if (coap_send(session, pdu) == COAP_INVALID_MID)
    die("cannot send the request", NULL);
  while (!exchange.finished)
    coap_io_process(ctx, 1000);
  coap_session_release(session);
  coap_free_context(ctx);
  if (exchange.failure)
    die(exchange.failure, NULL);
}

static void expect(coap_pdu_code_t code) {
  if (exchange.code != code) {
    char text[128];
    size_t n = exchange.payload_len < sizeof text - 1 ? exchange.payload_len
                                                      : sizeof text - 1;
    memcpy(text, exchange.payload, n);
    text[n] = '\0';
    fprintf(stderr, "todo-c: %d.%02d: %s\n", exchange.code >> 5,
            exchange.code & 0x1f, text);
    exit(1);
  }
}

int main(int argc, char **argv) {
  const char *host = "127.0.0.1", *port = "5683";
  const char *cmd[2] = {NULL, NULL};
  int ncmd = 0;
  for (int i = 1; i < argc; i++) {
    if (strcmp(argv[i], "--host") == 0 && i + 1 < argc)
      host = argv[++i];
    else if (strcmp(argv[i], "--port") == 0 && i + 1 < argc)
      port = argv[++i];
    else if (ncmd < 2)
      cmd[ncmd++] = argv[i];
    else
      ncmd = 3;
  }

  coap_startup();
  coap_set_log_level(COAP_LOG_WARN);
  load_schema();
  Bytes body;
  if (ncmd == 1 && strcmp(cmd[0], "list") == 0) {
    request(host, port, COAP_REQUEST_CODE_GET, NULL, NULL);
    expect(COAP_RESPONSE_CODE_CONTENT);
    print_list(exchange.payload, exchange.payload_len);
  } else if (ncmd == 2 && strcmp(cmd[0], "add") == 0) {
    encode_field("title", cmd[1], 0, &body);
    request(host, port, COAP_REQUEST_CODE_POST, NULL, &body);
    expect(COAP_RESPONSE_CODE_CREATED);
    printf("%s\n", exchange.location);
  } else if (ncmd == 2 && strcmp(cmd[0], "done") == 0) {
    encode_field("done", NULL, 1, &body);
    request(host, port, COAP_REQUEST_CODE_PUT, cmd[1], &body);
    expect(COAP_RESPONSE_CODE_CHANGED);
  } else if (ncmd == 2 && strcmp(cmd[0], "remove") == 0) {
    request(host, port, COAP_REQUEST_CODE_DELETE, cmd[1], NULL);
    expect(COAP_RESPONSE_CODE_DELETED);
  } else {
    fprintf(stderr, "usage: todo-c [--host H] [--port N] list | add <title> | "
                    "done <id> | remove <id>\n");
    return 2;
  }
  destroySchema(&senml_schema);
  coap_cleanup();
  return 0;
}
