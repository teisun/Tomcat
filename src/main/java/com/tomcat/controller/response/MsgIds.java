package com.tomcat.controller.response;

import lombok.Data;

/**
 * @projectName: Tomcat
 * @package: com.tomcat.controller.response
 * @className: UID
 * @author: tomcat
 * @description: TODO
 * @date: 2023/9/18 7:28 PM
 * @version: 1.0
 */

@Data
public class MsgIds {
    private String uid;
    private String chatId;
    private String msgId;

    public static MsgIds build(String uid, String chatId, String msgId){
        MsgIds msgIds = new MsgIds();
        msgIds.setUid(uid);
        msgIds.setChatId(chatId);
        msgIds.setMsgId(msgId);
        return msgIds;
    }

    public static MsgIds build(String uid, String chatId){
        return build(uid, chatId, null);
    }

    public static MsgIds build(String uid){
        return build(uid, null, null);
    }

}
