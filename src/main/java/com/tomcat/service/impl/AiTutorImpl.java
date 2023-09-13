package com.tomcat.service.impl;

import cn.hutool.core.lang.TypeReference;
import cn.hutool.core.util.StrUtil;
import cn.hutool.json.JSONUtil;
import com.tomcat.config.LocalCache;
import com.tomcat.controller.requeset.ChatReq;
import com.tomcat.controller.requeset.ChatUserDataReq;
import com.tomcat.controller.requeset.TipsReq;
import com.tomcat.controller.response.*;
import com.tomcat.service.AiCTutor;
import com.tomcat.utils.UniqueIdentifierGenerator;
import com.tomcat.websocket.Command;
import com.unfbx.chatgpt.OpenAiClient;
import com.unfbx.chatgpt.entity.chat.ChatCompletion;
import com.unfbx.chatgpt.entity.chat.ChatCompletionResponse;
import com.unfbx.chatgpt.entity.chat.Message;
import lombok.extern.slf4j.Slf4j;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.stereotype.Service;

import java.util.*;

/**
 * @projectName: Tomcat
 * @package: com.tomcat.service
 * @className: AiCLient
 * @author: tomcat
 * @description: TODO
 * @date: 2023/8/29 2:35 PM
 * @version: 1.0
 */

@Service
@Slf4j
public class AiTutorImpl implements AiCTutor {


    @Autowired
    private OpenAiClient aiClient;

    @Value("${ai.prompt.aitutor}")
    private String promptAitutor;
    @Value("${ai.prompt.version}")
    private String promptVersion;
    @Value("${ai.prompt.curriculum.limiter}")
    private String promptCurriculumLimiter;
    @Value("${ai.prompt.chat.limiter}")
    private String promptChatLimiter;

    @Value("${ai.prompt.chat.sentence_checker}")
    private String promptSentenceChecker;

    @Value("${ai.prompt.chat.generateTips}")
    private String promptChatGenerateTips;

    @Override
    public ChatCompletionResponse chatCompletion(List<Message> messages) {
        ChatCompletion completion = ChatCompletion
                .builder()
                .messages(messages)
                .model(ChatCompletion.Model.GPT_3_5_TURBO_16K.getName())
                .build();
        return aiClient.chatCompletion(completion);
    }

    @Override
    public ChatCompletionResponse chatCompletion(ChatCompletion chatCompletion) {
        chatCompletion.setModel(ChatCompletion.Model.GPT_3_5_TURBO_16K.getName());
        return aiClient.chatCompletion(chatCompletion);
    }

    @Override
    public List<Message> getChatContext(String uid) {
        return null;
    }

    @Override
    public ChatResp<String> chatInit(String uid) {

        Message firstMsg = Message.builder().content(promptAitutor).role(Message.Role.USER).build();
        List<Message> messages = new ArrayList<>();
        messages.add(firstMsg);
        Message secondMsg = Message.builder().content(promptVersion).role(Message.Role.ASSISTANT).build();
        messages.add(secondMsg);
        LocalCache.CACHE_INIT_MSG.put(uid, messages, LocalCache.TIMEOUT);
        log.info("chatInit: " + secondMsg.getContent());
        ChatResp<String> resp = new ChatResp<>();
        resp.setCode(200);
        resp.setCommand(Command.CHAT_INIT);
        resp.setData(uid);
        return resp;
    }

    @Override
    public ChatResp<String> chatInit(ChatReq req) {
        String contextStr = req.getData();
        String uid = req.getUid();
        String chatId = UniqueIdentifierGenerator.uniqueId();
        // 缓存初始化AI的上下文
        List<Message> context = JSONUtil.toBean(contextStr, new TypeReference<List<Message>>(){}, false);
        List<Message> initMessages = new ArrayList<>();
        initMessages.add(context.get(0));
        initMessages.add(context.get(1));
        initMessages.add(context.get(2));
        initMessages.add(context.get(3));
        LocalCache.CACHE_INIT_MSG.put(uid, initMessages, LocalCache.TIMEOUT);
        log.info("chatInit initMessages:" + JSONUtil.toJsonStr(initMessages));

        // 缓存用户与AI的场景对话聊天记录
        if(context.size()>=5){
            List<Message> chatMessages = new ArrayList<>(context);
            LocalCache.CACHE_CHAT_MSG.put(chatId, chatMessages);
            log.info("chatInit chatMessages:" + JSONUtil.toJsonStr(chatMessages));
        }


        // 保存chatId与uid的关系
        List<String> ids = (List<String>) LocalCache.CACHE_UID_CHATID.get(uid);
        if (ids == null) {
            ids = new ArrayList<>();
        }
        ids.add(chatId);
        if(LocalCache.CACHE_CHAT_MSG.containsKey(chatId)) LocalCache.CACHE_UID_CHATID.put(uid, ids);
        // ------

        ChatResp<String> resp = new ChatResp<>();
        resp.setCode(200);
        resp.setCommand(Command.CHAT_INIT_BY_CONTEXT);
        resp.setData(uid);
        if(LocalCache.CACHE_CHAT_MSG.containsKey(chatId)) resp.setChatId(chatId);
        return resp;
    }

    @Override
    public ChatResp<TopicsResp> curriculumPlan(ChatReq req) {
        String uid = req.getUid();
        // 获取chat上下文
        ChatResp<TopicsResp> resp = new ChatResp<>();
        List<Message> messages = (List<Message>) LocalCache.CACHE_INIT_MSG.get(uid);
        if (messages == null) {
            resp.setCode(404);
            resp.setDescribe("chat context not found!");
            return resp;
        }

        log.info(Command.CURRICULUM_PLAN + " messageContext: " + JSONUtil.toJsonStr(messages));
        String content;
        String prompt_limit = " " + promptCurriculumLimiter; // 限时模型返回topic obj的数量
        if (StrUtil.isNotBlank(req.getData())) {
            content = Command.CURRICULUM_PLAN + " " + req.getData() + prompt_limit;
        } else {
            content = Command.CURRICULUM_PLAN + prompt_limit;
        }
        log.info(Command.CURRICULUM_PLAN + " currentMessage content: " + content);
        Message currentMessage = Message.builder().content(content).role(Message.Role.USER).build();
        messages.add(currentMessage);
        ChatCompletionResponse response = this.chatCompletion(messages);
        Message responseMag = response.getChoices().get(0).getMessage();
        log.info(Command.CURRICULUM_PLAN + " responseMag content: " + responseMag.getContent());

        TopicsResp topicsResp = JSONUtil.toBean(responseMag.getContent(), TopicsResp.class);
        resp.setCode(200);
        resp.setCommand(Command.CURRICULUM_PLAN);
        resp.setData(topicsResp);
        resp.setUsage(response.getUsage());

        // 将响应数据加入到上下文缓存中
        messages.add(responseMag);
        LocalCache.CACHE_INIT_MSG.put(uid, messages, LocalCache.TIMEOUT);

        return resp;
    }

    @Override
    public ChatResp<ChatAssistantDataResp> startTopic(ChatReq req) {

        // 保存chatId与uid的关系
        String uid = req.getUid();
        List<String> ids = (List<String>) LocalCache.CACHE_UID_CHATID.get(uid);
        if (ids == null) {
            ids = new ArrayList<>();
        }
        String chatId = UniqueIdentifierGenerator.uniqueId();
        ids.add(chatId);
        LocalCache.CACHE_UID_CHATID.put(uid, ids);
        // ------

        // 拼接聊天上下文
        List<Message> initMessages = (List<Message>) LocalCache.CACHE_INIT_MSG.get(uid);
        ChatResp<ChatAssistantDataResp> resp = new ChatResp<>();
        if (initMessages == null) {
            resp.setCode(404);
            resp.setDescribe("chat context not found!");
            return resp;
        }

        List<Message> chatMessages = new ArrayList<>(initMessages);

        log.info(Command.START_TOPIC + " messageContext: " + JSONUtil.toJsonStr(chatMessages));
        // 编辑prompt加入到上下文中
        String content;
        if (StrUtil.isNotBlank(req.getData())) {
            content = Command.START_TOPIC + " " + req.getData();
        } else {
            content = Command.START_TOPIC + " random";
        }
        log.info(Command.START_TOPIC + " currentMessage content: " + content);
        Message currentMessage = Message.builder().content(content).role(Message.Role.USER).build();
        chatMessages.add(currentMessage);
        // ------

        // 发送上下文到AI 获取返回的响应数据
        ChatCompletionResponse response = this.chatCompletion(chatMessages);
        Message responseMag = response.getChoices().get(0).getMessage();
        log.info(Command.START_TOPIC + "  responseMag content: " + responseMag.getContent());

        // 将响应数据格式化成java bean返回请求端
        ChatAssistantDataResp chatAssistantDataResp = JSONUtil.toBean(responseMag.getContent(), ChatAssistantDataResp.class);

        // 生成tips
        TipsReq tipsReq = new TipsReq();
        tipsReq.setTopic(chatAssistantDataResp.getTopic());
        tipsReq.setQuestion(chatAssistantDataResp.getAssistant_sentence());
        TipsResp tipsResp = generateTips(JSONUtil.toJsonStr(tipsReq));
        chatAssistantDataResp.setTips(tipsResp.getTips());
        // ------

        resp.setCode(200);
        resp.setCommand(Command.START_TOPIC);
        resp.setData(chatAssistantDataResp);
        resp.setUsage(response.getUsage());
        resp.addUsage(tipsResp.getUsage());
        resp.setChatId(chatId);
        resp.setMsgId(response.getId());

        // 将响应数据加入到上下文缓存中
        chatMessages.add(responseMag);
        LocalCache.CACHE_CHAT_MSG.put(chatId, chatMessages, LocalCache.TIMEOUT);
        cacheOfflineMsg(req.getUid(), resp);
        return resp;
    }

    @Override
    public ChatResp<ChatAssistantDataResp> chat(ChatReq req) {

        String chatId = req.getChatId();

        // 获得chat上下文
        List<Message> chatMessages = (List<Message>) LocalCache.CACHE_CHAT_MSG.get(chatId);
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
        log.info(Command.CHAT + " messageContext: " + JSONUtil.toJsonStr(chatMessages));
        // 检查user_sentence语法
        Message messageCheckSentence = Message.builder().content("sentence:" + req.getData() + " \n" + promptSentenceChecker).role(Message.Role.USER).build();
        List<Message> messagesCheckSentence = new ArrayList<>();
        messagesCheckSentence.add(messageCheckSentence);
        ChatCompletionResponse checkSentenceResponse = this.chatCompletion(messagesCheckSentence);
        String checkSentenceResponseStr = checkSentenceResponse.getChoices().get(0).getMessage().getContent();
        ChatAssistantDataResp.Suggestion suggestion = JSONUtil.toBean(checkSentenceResponseStr, ChatAssistantDataResp.Suggestion.class);
        log.info(Command.CHAT + " checkSentenceResponse content: " + checkSentenceResponseStr);
        log.info(Command.CHAT + " checkSentenceResponse usage: " + checkSentenceResponse.getUsage());

        // 编辑prompt加入到上下文中
        ChatUserDataReq chatUserDataReq = new ChatUserDataReq();
        chatUserDataReq.setUser_sentence(req.getData());
        chatUserDataReq.setPrompt(promptChatLimiter);
        String content = JSONUtil.toJsonStr(chatUserDataReq);
        log.info(Command.CHAT + " currentMessage content: " + content);
        Message currentMessage = Message.builder().content(content).role(Message.Role.USER).build();
        chatMessages.add(currentMessage);
        // 发送上下文到AI 获取返回的响应数据
        ChatCompletionResponse response = this.chatCompletion(chatMessages);
        Message responseMag = response.getChoices().get(0).getMessage();
        log.info(Command.CHAT + " responseMag content: " + responseMag.getContent());

        // 将响应数据格式化成java bean返回请求端
        ChatAssistantDataResp chatAssistantDataResp = JSONUtil.toBean(responseMag.getContent(), ChatAssistantDataResp.class);

        // 生成tips
        TipsReq tipsReq = new TipsReq();
        tipsReq.setTopic(chatAssistantDataResp.getTopic());
        tipsReq.setQuestion(chatAssistantDataResp.getAssistant_sentence());
        TipsResp tipsResp = generateTips(JSONUtil.toJsonStr(tipsReq));

        chatAssistantDataResp.setTips(tipsResp.getTips());
        chatAssistantDataResp.setSuggestion(suggestion);
        resp.setCode(200);
        resp.setCommand(Command.CHAT);
        resp.setData(chatAssistantDataResp);
        resp.setUsage(response.getUsage());
        resp.addUsage(tipsResp.getUsage());
        resp.setChatId(chatId);
        resp.setMsgId(response.getId());

        // 将响应数据加入到上下文缓存中
        chatMessages.add(responseMag);
        LocalCache.CACHE_CHAT_MSG.put(chatId, chatMessages, LocalCache.TIMEOUT);
        cacheOfflineMsg(req.getUid(), resp);
        return resp;
    }

    @Override
    public ChatResp<TipsResp> generateTips(ChatReq req) {
        ChatResp<TipsResp> resp = new ChatResp<>();
        if (StrUtil.isBlank(req.getData())) {
            resp.setCode(404);
            resp.setDescribe("Req data must be not null!");
            return resp;
        }
        TipsResp tipsResp = generateTips(req.getData());
        resp.setCode(200);
        resp.setCommand(Command.TIPS);
        resp.setData(tipsResp);
        resp.setUsage(tipsResp.getUsage());

        return resp;
    }

    @Override
    public ChatResp<List<OfflineMsgResp>> offlineMsg(ChatReq req) {
        log.info(Command.OFFLINE_MSG + " req data: " + req.getData());
        ChatResp<List<OfflineMsgResp>> resp = new ChatResp<>();
        if (!LocalCache.CACHE_OFFLINE_MSG.containsKey(req.getUid())) {
            resp.setCode(404);
            resp.setDescribe("user:" + req.getUid() + " has no offline messages!");
            return resp;
        }
        Map<String, OfflineMsgResp> map = (Map<String, OfflineMsgResp>) LocalCache.CACHE_OFFLINE_MSG.get(req.getUid());
        List<OfflineMsgResp> list = new ArrayList<>(map.values());
        resp.setData(list);
        resp.setCode(200);
        resp.setCommand(Command.OFFLINE_MSG);
        LocalCache.CACHE_OFFLINE_MSG.remove(req.getUid());
        return resp;
    }

    @Override
    public ChatResp msgConfirm(ChatReq req) {
        log.info(Command.MSG_CONFIRM + " req data:" +req.getData());
        boolean completion = false;
        if(LocalCache.CACHE_OFFLINE_MSG.containsKey(req.getUid())){
            Map<String, OfflineMsgResp> map = (Map<String, OfflineMsgResp>) LocalCache.CACHE_OFFLINE_MSG.get(req.getUid());
            if(map.remove(req.getData()) != null){
                completion = true;
            }
        }
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

    private TipsResp generateTips(String tipsReq) {
        Message messageGenerateTips = Message.builder().content(tipsReq + "\n" + promptChatGenerateTips).role(Message.Role.USER).build();
        List<Message> messages = new ArrayList<>();
        messages.add(messageGenerateTips);
        ChatCompletionResponse response = this.chatCompletion(messages);
        String responseStr = response.getChoices().get(0).getMessage().getContent();
        TipsResp tipsResp = JSONUtil.toBean(responseStr, TipsResp.class);
        tipsResp.setUsage(response.getUsage());
        log.info(Command.TIPS + " " + tipsReq);
        log.info(Command.TIPS + " generateTips content: " + responseStr);
        log.info(Command.TIPS + " generateTips usage: " + response.getUsage());
        return tipsResp;
    }


}
