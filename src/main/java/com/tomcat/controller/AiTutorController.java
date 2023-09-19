package com.tomcat.controller;

import cn.hutool.core.util.StrUtil;
import cn.hutool.json.JSONUtil;
import com.tomcat.config.LocalCache;
import com.tomcat.controller.requeset.ChatReq;
import com.tomcat.controller.response.*;
import com.tomcat.service.AiCTutor;
import com.tomcat.utils.ThreeTuple;
import com.tomcat.utils.TwoTuple;
import com.tomcat.websocket.CmdMapping;
import com.tomcat.websocket.Command;
import com.unfbx.chatgpt.entity.chat.Message;
import com.unfbx.chatgpt.entity.common.Usage;
import lombok.extern.slf4j.Slf4j;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.stereotype.Component;

import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * @projectName: Tomcat
 * @package: com.tomcat.websocket
 * @className: AiTutorController
 * @author: tomcat
 * @description: TODO
 * @date: 2023/9/18 4:22 PM
 * @version: 1.0
 */
@Component
@Slf4j
public class AiTutorController {

    @Autowired
    AiCTutor aiTutor;

    @CmdMapping(Command.CHAT_INIT)
    public ChatResp chatInit(ChatReq req) {
        MsgIds ids = aiTutor.chatInit(req.getUid());
        ChatResp<String> resp = new ChatResp<>();
        resp.setCode(200);
        resp.setCommand(Command.CHAT_INIT);
        resp.setData(ids.getUid());
        return resp;
    }

    @CmdMapping(Command.CHAT_INIT_BY_CONTEXT)
    public ChatResp chatInitByContext(ChatReq req) {

        MsgIds ids = aiTutor.chatInit(req);

        ChatResp<String> resp = new ChatResp<>();
        resp.setCode(200);
        resp.setCommand(Command.CHAT_INIT_BY_CONTEXT);
        resp.setData(ids.getUid());
        resp.setChatId(ids.getChatId());

        return resp;
    }

    @CmdMapping(Command.PLAN)
    public ChatResp plan(ChatReq req) {
        List<Message> messages = (List<Message>) LocalCache.CACHE_INIT_MSG.get(req.getUid());
        if (messages == null) {
            ChatResp resp = new ChatResp<>();
            resp.setCode(404);
            resp.setDescribe("chat context not found!");
            return resp;
        }
        TwoTuple<TopicsResp, Usage> twoTuple = aiTutor.plan(req);
        ChatResp<TopicsResp> resp = new ChatResp<>();
        resp.setCode(200);
        resp.setCommand(Command.PLAN);
        resp.setData(twoTuple.getFirst());
        resp.setUsage(twoTuple.getSecond());
        return resp;
    }

    @CmdMapping(Command.START_TOPIC)
    public ChatResp startTopic(ChatReq req) {
        ChatResp<ChatAssistantDataResp> resp = new ChatResp<>();
        List<Message> initMessages = (List<Message>) LocalCache.CACHE_INIT_MSG.get(req.getUid());
        if (initMessages == null) {
            resp.setCode(404);
            resp.setDescribe("chat context not found!");
            return resp;
        }
        ThreeTuple<ChatAssistantDataResp, MsgIds, Usage> tuple = aiTutor.startTopic(req);
        MsgIds msgIds = tuple.getSecond();
        resp.setCode(200);
        resp.setCommand(Command.START_TOPIC);
        resp.setData(tuple.getFirst());
        resp.setChatId(msgIds.getChatId());
        resp.setMsgId(msgIds.getMsgId());
        resp.setUsage(tuple.getThird());
        cacheOfflineMsg(req.getUid(), resp);
        return resp;
    }

    @CmdMapping(Command.CUSTOMIZE_TOPIC)
    public ChatResp customizeTopic(ChatReq req){
        ChatResp<ChatAssistantDataResp> resp = new ChatResp<>();
        List<Message> initMessages = (List<Message>) LocalCache.CACHE_INIT_MSG.get(req.getUid());
        if (StrUtil.isBlank(req.getData())) {
            resp.setCode(404);
            resp.setDescribe("req data must be not null!");
            return resp;
        }

        if (initMessages == null) {
            resp.setCode(404);
            resp.setDescribe("chat context not found!");
            return resp;
        }
        ThreeTuple<ChatAssistantDataResp, MsgIds, Usage> tuple = aiTutor.customizeTopic(req);
        MsgIds ids = tuple.getSecond();
        resp.setCode(200);
        resp.setCommand(Command.CUSTOMIZE_TOPIC);
        resp.setData(tuple.getFirst());
        resp.setChatId(ids.getChatId());
        resp.setMsgId(ids.getMsgId());
        resp.setUsage(tuple.getThird());

        cacheOfflineMsg(req.getUid(), resp);

        return resp;
    }

    @CmdMapping(Command.CHAT)
    public ChatResp chat(ChatReq req) {
        List<Message> chatMessages = (List<Message>) LocalCache.CACHE_CHAT_MSG.get(req.getChatId());
        ChatResp<ChatAssistantDataResp> resp = new ChatResp<>();
        if (StrUtil.isBlank(req.getData())) {
            resp.setCode(404);
            resp.setDescribe("chat data must be not null!");
            return resp;
        }
        if (chatMessages == null) {
            resp.setCode(404);
            resp.setDescribe("chat context not found!");
            return resp;
        }
        ThreeTuple<ChatAssistantDataResp, MsgIds, Usage> tuple = aiTutor.chat(req);
        MsgIds ids = tuple.getSecond();
        resp.setCode(200);
        resp.setCommand(Command.CHAT);
        resp.setData(tuple.getFirst());
        resp.setUsage(tuple.getThird());
        resp.setChatId(ids.getChatId());
        resp.setMsgId(ids.getMsgId());
        cacheOfflineMsg(req.getUid(), resp);
        return resp;
    }

    @CmdMapping(Command.TIPS)
    public ChatResp generateTips(ChatReq req) {
        ChatResp<TipsResp> resp = new ChatResp<>();
        if (StrUtil.isBlank(req.getData())) {
            resp.setCode(404);
            resp.setDescribe("Req data must be not null!");
            return resp;
        }
        TwoTuple<TipsResp, Usage> tuple = aiTutor.generateTips(req);
        resp.setCode(200);
        resp.setCommand(Command.TIPS);
        resp.setData(tuple.getFirst());
        resp.setUsage(tuple.getSecond());
        return resp;
    }

    @CmdMapping(Command.OFFLINE_MSG)
    public ChatResp<List<OfflineMsgResp>> offlineMsg(ChatReq req) {
        ChatResp<List<OfflineMsgResp>> resp = new ChatResp<>();
        if (!LocalCache.CACHE_OFFLINE_MSG.containsKey(req.getUid())) {
            resp.setCode(404);
            resp.setDescribe("user:" + req.getUid() + " has no offline messages!");
            return resp;
        }
        List<OfflineMsgResp> offlineMsgResps = aiTutor.offlineMsg(req);
        resp.setData(offlineMsgResps);
        resp.setCode(200);
        resp.setCommand(Command.OFFLINE_MSG);
        return resp;
    }

    @CmdMapping(Command.MSG_CONFIRM)
    public ChatResp msgConfirm(ChatReq req) {
        boolean completion = aiTutor.msgConfirm(req);
        ChatResp resp = new ChatResp();
        resp.setCommand(Command.MSG_CONFIRM);
        if(completion){
            resp.setCode(200);
            resp.setDescribe("msg confirmed");
        }else {
            resp.setCode(404);
            resp.setDescribe("This message was not found on the server");
        }
        return resp;
    }


    private void cacheOfflineMsg(String uid, ChatResp resp) {
        log.info("MessageProcessor sendText: " + resp.getCommand() + " cache the message!");
        Map<String, OfflineMsgResp> msgMap;
        if(LocalCache.CACHE_OFFLINE_MSG.containsKey(uid)){
            msgMap = (Map<String, OfflineMsgResp>) LocalCache.CACHE_OFFLINE_MSG.get(uid);
        }else {
            msgMap = new HashMap<>();
        }
        OfflineMsgResp offlineMsg = new OfflineMsgResp();
        offlineMsg.setChatId(resp.getChatId());
        offlineMsg.setMsg(JSONUtil.toJsonStr(resp.getData()));
        msgMap.put(resp.getMsgId(), offlineMsg);
        LocalCache.CACHE_OFFLINE_MSG.put(uid, msgMap);
    }
}
